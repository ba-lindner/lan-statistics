use std::{fs::File, io::Write, fmt::Write as _};

use config::Config;
use log::warn;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Clone)]
pub struct Settings {
    pub id: String,
    pub remote: String,
    pub name: Option<String>,
    pub autostart: bool,
    pub password: Option<String>
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            remote: String::from("https://lan.pein-gera.de"),
            name: None,
            autostart: true,
            password: None
        }
    }
}

const PASSWORD_PLACEHOLDER: &str = "(unchanged)";
pub const CONFIG_PATH: &str = "config.toml";

pub fn get_config(censor: bool) -> Result<Settings, String> {
    Config::builder()
        .add_source(config::File::with_name(CONFIG_PATH))
        .build()
        .map_err(|e| e.to_string())?
        .try_deserialize::<Settings>()
        .map(|mut c| {
            if censor {
                c.password = c.password
                    .map(|_| String::from(PASSWORD_PLACEHOLDER));
                c
            } else {
                c
            }
        })
        .map_err(|e| e.to_string())
}

pub fn set_config(config: &Settings) -> Result<(), String> {
    let password = config.password.clone().map(|p|
        if p == PASSWORD_PLACEHOLDER {
            Ok::<Option<String>, String>(get_config(false).ok()
                .and_then(|c| c.password))
        } else {
            let mut hasher = Sha256::new();
            hasher.update(p);
            let mut out = String::new();
            write!(out, "{:x}", hasher.finalize()).map_err(|e| e.to_string())?;
            Ok(Some(out))
        }
    ).unwrap_or(Ok(None))?;
    let mut new_config = config.clone();
    new_config.password = password;

    let mut file = File::create(CONFIG_PATH).map_err(|e| e.to_string())?;
    let content = toml::to_string_pretty(&new_config).map_err(|e| e.to_string())?;

    file.write(content.as_bytes()).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn create_default_config() -> Result<Settings, String> {
    let default = Settings::default();

    set_config(&default)?;

    Ok(default)
}

pub fn get_or_create_config(censor: bool) -> Result<Settings, String> {
    get_config(censor).or_else(|e| {
        warn!("error getting config: {e}");
        create_default_config()
    })
}