mod hardware;

use crate::consts::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use sha2::{Sha256, Digest};
use std::path::PathBuf;
use std::env;

pub use hardware::HardwareCollector;

#[derive(Debug, Serialize, Deserialize)]
pub struct GpuInfo {
    pub model: String,
    pub memory: u64,
    pub cuda_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu_info: String,
    pub gpu_did: String,
    pub driver_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub cpu_serial: String,
    pub gpu_uuid: Option<String>,
    pub system_fingerprint: String,
    pub installation_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceInitRequest {
    pub device_fingerprint: String,
    pub gpu_info: GpuInfo,
    pub hardware_info: HardwareInfo,
    pub installation_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceInitResponse {
    pub device_code: String,
    pub verification_uri: String,
    pub user_code: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceVerifyResponse {
    pub node_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("设备初始化失败: {0}")]
    InitError(String),
    #[error("设备验证失败: {0}")]
    VerifyError(String),
    #[error("设备码过期")]
    CodeExpired,
    #[error("设备已被禁用")]
    DeviceDisabled,
    #[error("网络错误: {0}")]
    NetworkError(String),
}

pub struct DeviceManager {
    client: reqwest::Client,
    base_url: String,
}

impl DeviceManager {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    pub async fn init_device(
        &self,
        device_info: DeviceInfo,
        gpu_info: GpuInfo,
        hardware_info: HardwareInfo,
    ) -> Result<DeviceInitResponse, DeviceError> {
        let fingerprint = self.generate_device_fingerprint(&device_info);
        let request = DeviceInitRequest {
            device_fingerprint: fingerprint,
            gpu_info,
            hardware_info,
            installation_hash: device_info.installation_hash,
        };

        log::debug!(
            "Device init request body: {}",
            serde_json::to_string_pretty(&request).unwrap()
        );

        let response = self
            .client
            .post(format!("{}{}", self.base_url, API_NODES_INIT))
            .json(&request)
            .send()
            .await
            .map_err(|e| DeviceError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DeviceError::InitError(format!(
                "{}: {}",
                ERROR_DEVICE_INIT_FAILED,
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| DeviceError::InitError(e.to_string()))
    }

    pub async fn verify_device(
        &self,
        user_code: &str,
    ) -> Result<DeviceVerifyResponse, DeviceError> {
        log::debug!("Device verify request user_code: {}", user_code);

        let response = self
            .client
            .get(format!(
                "{}{}/{}",
                self.base_url, API_NODES_VERIFY, user_code
            ))
            .send()
            .await
            .map_err(|e| DeviceError::NetworkError(e.to_string()))?;

        match response.status() {
            reqwest::StatusCode::OK => response
                .json()
                .await
                .map_err(|e| DeviceError::VerifyError(e.to_string())),
            reqwest::StatusCode::GONE => Err(DeviceError::CodeExpired),
            reqwest::StatusCode::FORBIDDEN => Err(DeviceError::DeviceDisabled),
            _ => Err(DeviceError::VerifyError(format!(
                "{}: {}",
                ERROR_DEVICE_VERIFY_FAILED,
                response.status()
            ))),
        }
    }

    fn generate_device_fingerprint(&self, info: &DeviceInfo) -> String {
        // Create a combined string from hardware info and gpu info
        let combined = format!(
            "{}:{}:{}:{}",
            info.cpu_serial,
            info.gpu_uuid.as_deref().unwrap_or("unknown"),
            info.system_fingerprint,
            info.installation_hash
        );
        
        // Generate SHA-256 hash
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        let result = hasher.finalize();
        
        // Convert to hex string
        format!("{:x}", result)
    }

    pub fn generate_installation_hash(&self) -> String {
        // Dummy hash for development
        "dummy_installation_hash_2024".to_string()
    }
}
