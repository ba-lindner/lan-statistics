// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, error::Error, process::Command};

use log::{error, info, warn};
use named_lock::NamedLock;
mod app;
mod config;
mod metrics;

const USAGE: &str = "\
usage: lan-tracker.exe [-s|--service] [-h|--help]\n\
    -s|--service: start background task\n\
    -h|--help:   print this usage information";

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::current_exe()?
        .parent()
        .ok_or(String::from("could not get program folder"))?
        .to_path_buf();
    env::set_current_dir(path)?;

    log4rs::init_file("log4rs.yml", Default::default())?;

    let mut is_service = false;

    for arg in env::args() {
        match arg.as_str() {
            "-s" | "--service" => is_service = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => {}
        }
    }

    if is_service {
        match NamedLock::create("lan-tracker")?.try_lock() {
            Ok(_) => (),
            Err(named_lock::Error::WouldBlock) => {
                warn!("could not get lock. Another instance is already running.");
                return Ok(());
            }
            Err(e) => Err(e)?,
        };

        info!("starting service");

        let Err(why) = metrics::metrics_loop();
        error!("error in metrics loop: {why}");
    } else {
        if let Err(why) = Command::new(env::current_exe()?).args(["-s"]).spawn() {
            error!("error spawning service: {why}")
        }

        app::run();
    }
    Ok(())
}
