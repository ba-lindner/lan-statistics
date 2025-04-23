use std::{collections::HashMap, convert::Infallible, error::Error, thread::sleep, time::Duration};

use clokwerk::{Scheduler, TimeUnits};
use log::{error, info};
use prometheus::{core::Collector, push_metrics, BasicAuthentication, CounterVec, Gauge, GaugeVec, Opts, Registry};
use sysinfo::{Networks, ProcessesToUpdate, System, MINIMUM_CPU_UPDATE_INTERVAL};

use crate::config::{get_or_create_config, Settings};

fn new_opts(config: &Settings, name: &'static str, help: &'static str) -> Opts {
    Opts::new(name, help)
        .const_label("id", &config.id)
        .const_label("username", config.name.clone().unwrap_or_default())
}

fn new_metric<T: Collector + Clone + 'static, U: Error>(r: &Registry, metric: Result<T, U>) -> Result<T, String> {
    let unwrapped = metric.map_err(|e| e.to_string())?;

    r.register(Box::new(unwrapped.clone())).map_err(|e| e.to_string())?;

    Ok(unwrapped)
}

fn new_gauge_labeled(
    config: &Settings,
    r: &Registry,
    name: &'static str,
    help: &'static str,
    labeler: impl Fn(Opts) -> Opts,
) -> Result<Gauge, String> {
    let gauge = new_metric(r, Gauge::with_opts(labeler(new_opts(config, name, help))))?;

    Ok(gauge)
}

fn new_gauge(config: &Settings, r: &Registry, name: &'static str, help: &'static str) -> Result<Gauge, String> {
    new_gauge_labeled(config, r, name, help, |o| o)
}

fn send_metrics(config: &Settings, r: &Registry) -> Result<(), String> {
    push_metrics(
        "lan-tracker",
        HashMap::new(),
        &config.remote,
        r.gather(),
        config.password.clone().map(|p| BasicAuthentication {
            username: String::from("lan-tracker"),
            password: p
        }),
    ).map_err(|e| e.to_string())
}

fn register_one_time(config: &Settings, r: &Registry) -> Result<(), String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let system_info = new_gauge_labeled(
        config,
        r,
        "system_info",
        "provides information about the system",
        |opts| {
            opts.const_label("name", System::name().unwrap_or(String::from("")))
                .const_label("kernel", System::kernel_long_version())
                .const_label(
                    "version",
                    System::long_os_version()
                        .or(System::os_version())
                        .unwrap_or(String::from("")),
                )
                .const_label("hostname", System::host_name().unwrap_or(String::from("")))
                .const_label(
                    "cpu_name",
                    String::from(
                        sys.cpus()
                            .first()
                            .map(|cpu| cpu.brand().trim())
                            .unwrap_or(""),
                    ),
                )
                .const_label(
                    "cpu_vendor",
                    String::from(sys.cpus().first().map(|cpu| cpu.vendor_id()).unwrap_or("")),
                )
        },
    )?;

    let cpu_core_count = new_gauge(config, r, "cpu_core_count", "displays cpu core count")?;
    let max_memory_bytes = new_gauge(config, r, "max_memory_bytes", "displays memory capacity")?;

    system_info.set(1.0);
    cpu_core_count.set(sys.cpus().len() as f64);
    max_memory_bytes.set(sys.total_memory() as f64);

    Ok(())
}

fn register_periodic(config: &Settings, r: &Registry) -> Result<impl FnMut(), String> {
    let mut networks = Networks::new();
    let mut sys = System::new();

    let network_info = new_metric(
        r,
        GaugeVec::new(
            new_opts(
                config, 
                "network_info",
                "provides information about the network interfaces",
            ),
            &["name", "ip", "mac"],
        ),
    )?;

    let cpu_usage = new_gauge(config, r, "cpu_usage_percent", "displays cpu usage")?;
    let memory_usage = new_gauge(config, r, "memory_usage_bytes", "displays memory usage")?;

    let running_processes = new_metric(
        r,
        CounterVec::new(
            new_opts(
                config,
                "running_processes_minutes",
                "provides information about running processes",
            ),
            &["name", "pid", "cwd", "exe"],
        ),
    )?;

    Ok(move || {
        network_info.reset();
        networks.refresh(true);

        sys.refresh_cpu_usage();
        sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_cpu_usage();

        sys.refresh_memory();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        for (interface, data) in networks.iter() {
            network_info
                .with_label_values(&[
                    interface,
                    &data
                        .ip_networks()
                        .iter()
                        .find_map(|ip| {
                            if ip.addr.is_ipv4() {
                                Some(ip.addr.to_string())
                            } else {
                                None
                            }
                        })
                        .unwrap_or(String::from("")),
                    &data.mac_address().to_string(),
                ])
                .set(1.0);
        }

        cpu_usage.set(sys.global_cpu_usage() as f64);
        memory_usage.set((sys.total_memory() - sys.available_memory()) as f64);

        for (pid, process) in sys.processes() {
            running_processes
                .with_label_values(&[
                    process.name().to_str().unwrap_or(""),
                    &pid.to_string(),
                    process.cwd().and_then(|p| p.to_str()).unwrap_or(""),
                    process.exe().and_then(|p| p.to_str()).unwrap_or(""),
                ])
                .inc();
        }
    })
}

pub fn metrics_loop() -> Result<Infallible, String> {
    let r = Registry::new();
    let config = get_or_create_config(false)?;

    register_one_time(&config, &r)?;

    let mut collector = register_periodic(&config, &r)?;

    let mut s = Scheduler::new();

    s.every(1.minute()).run(move || {
        get_or_create_config(false).and_then(|c| {
            info!("collecting and sending metrics");
            collector();
            send_metrics(&c, &r)
        }).unwrap_or_else(|e| error!("error in metrics loop: {e}"));
    });

    loop {
        s.run_pending();
        sleep(Duration::from_millis(10));
    }
}