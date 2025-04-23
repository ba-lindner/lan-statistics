use std::{fs::File, io::Write};

use anyhow::{Context, Result};
use config::Config;
use log::warn;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Clone)]
pub struct Settings {
    pub id: String,
    pub remote: String,
    pub name: Option<String>,
    pub autostart: bool,
    pub password: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            remote: String::from("https://lan.pein-gera.de"),
            name: None,
            autostart: true,
            password: None,
        }
    }
}

impl Settings {
    const PASSWORD_PLACEHOLDER: &str = "(unchanged)";
    pub const CONFIG_PATH: &str = "config.toml";

    pub fn load(censor: bool) -> Result<Self> {
        let mut settings = Config::builder()
            .add_source(config::File::with_name(Self::CONFIG_PATH))
            .build()
            .context("Failed to read config file")?
            .try_deserialize::<Settings>()
            .context("Failed to parse config file")?;
        if let (true, Some(pwd)) = (censor, &mut settings.password) {
            *pwd = Self::PASSWORD_PLACEHOLDER.to_string();
        }
        Ok(settings)
    }

    pub fn store(&self) -> Result<()> {
        let mut new_config = self.clone();
        new_config.password = match new_config.password.as_deref() {
            Some(Self::PASSWORD_PLACEHOLDER) => Settings::load(false)?.password,
            Some(changed) => {
                let mut hasher = Sha256::new();
                hasher.update(changed);
                Some(format!("{:x}", hasher.finalize()))
            }
            None => None,
        };

        let mut file = File::create(Self::CONFIG_PATH)?;
        let content = toml::to_string_pretty(&new_config)?;

        file.write_all(content.as_bytes())?;

        Ok(())
    }

    pub fn create_default() -> Result<Self> {
        let this = Self::default();
        this.store()?;
        Ok(this)
    }

    pub fn load_or_create(censor: bool) -> Result<Self> {
        Self::load(censor).or_else(|e| {
            warn!("error getting config: {e}");
            Self::create_default()
        })
    }
}
