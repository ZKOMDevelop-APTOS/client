use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use dirs::config_dir;
use crate::consts::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    pub device_code: Option<String>,
    pub user_code: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub node_id: Option<String>,
    pub base_url: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            device_code: None,
            user_code: None,
            access_token: None,
            refresh_token: None,
            node_id: None,
            base_url: API_BASE_URL.to_string(),
        }
    }
}

pub struct ConfigManager {
    config_path: PathBuf,
    config: NodeConfig,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config_dir = config_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        let config_path = config_dir.join(CONFIG_DIR).join(CONFIG_FILE);

        let config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            serde_json::from_str(&content)?
        } else {
            NodeConfig::default()
        };

        Ok(Self {
            config_path,
            config,
        })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(&self.config_path, content)?;
        Ok(())
    }

    pub fn get_config(&self) -> &NodeConfig {
        &self.config
    }

    #[allow(dead_code)]
    pub fn update_config(&mut self, new_config: NodeConfig) -> Result<()> {
        self.config = new_config;
        self.save()?;
        Ok(())
    }

    pub fn set_device_code(&mut self, code: String) -> Result<()> {
        self.config.device_code = Some(code);
        self.save()?;
        Ok(())
    }

    pub fn set_user_code(&mut self, code: String) -> Result<()> {
        self.config.user_code = Some(code);
        self.save()?;
        Ok(())
    }

    pub fn set_tokens(&mut self, access_token: String, refresh_token: String) -> Result<()> {
        self.config.access_token = Some(access_token);
        self.config.refresh_token = Some(refresh_token);
        self.save()?;
        Ok(())
    }

    pub fn set_node_id(&mut self, id: String) -> Result<()> {
        self.config.node_id = Some(id);
        self.save()?;
        Ok(())
    }

    pub fn update_access_token(&mut self, access_token: String) -> Result<()> {
        self.config.access_token = Some(access_token);
        self.save()?;
        Ok(())
    }
} 