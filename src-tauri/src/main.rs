// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    env,
    process::{Command, ExitCode},
};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use log::{error, info, warn};
use named_lock::NamedLock;

mod app;
mod config;
mod metrics;

#[derive(Parser)]
struct Cli {
    /// start background task
    #[arg(short, long)]
    service: bool,
}

fn main() -> ExitCode {
    let is_service = Cli::parse().service;

    if let Err(e) = setup_logging() {
        eprintln!("Failed to initialize logging: {e}");
        return ExitCode::FAILURE;
    };

    match run_service(is_service) {
        Err(e) => {
            error!("{e}");
            ExitCode::FAILURE
        }
        Ok(()) => ExitCode::SUCCESS,
    }
}

fn setup_logging() -> Result<()> {
    let path = env::current_exe()?
        .parent()
        .ok_or(anyhow!("Failed to get program folder"))?
        .to_path_buf();
    env::set_current_dir(path)?;

    log4rs::init_file("log4rs.yml", Default::default())?;
    Ok(())
}

fn run_service(is_service: bool) -> Result<()> {
    if is_service {
        match NamedLock::create("lan-tracker")?.try_lock() {
            Ok(_) => (),
            Err(named_lock::Error::WouldBlock) => {
                warn!("could not get lock. Another instance is already running.");
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };

        info!("starting service");

        match metrics::metrics_loop().context("Failed to start service")? {}
    } else {
        if let Err(why) = Command::new(env::current_exe()?).args(["-s"]).spawn() {
            error!("error spawning service: {why}");
        }

        app::run();
        Ok(())
    }
}
