use crate::consts::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose};

pub use hardware::{HardwareCollector, HardwareInfo};
pub mod hardware;

#[derive(Debug, Serialize, Deserialize)]
pub struct GpuInfo {
    pub model: String,
    pub memory: u64,
    pub cuda_version: String,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceMetrics {
    pub gpu_utilization: u8,      // GPU利用率（%）
    pub gpu_memory_used: u64,     // 显存使用量（MB）
    pub gpu_temperature: u8,      // GPU温度
    pub timestamp: String,        // ISO 8601格式的时间戳
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceHeartbeatRequest {
    pub node_id: String,
    pub metrics: DeviceMetrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceHeartbeatResponse {
    pub status: String,
    pub message: String,
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
    #[error("心跳发送失败: {0}")]
    HeartbeatError(String),
    #[error("令牌刷新失败: {0}")]
    RefreshError(String),
    #[error("令牌解析失败: {0}")]
    TokenParseError(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceRefreshResponse {
    pub access_token: String,
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

    pub async fn send_heartbeat(
        &self,
        node_id: &str,
        metrics: DeviceMetrics,
        access_token: &str,
    ) -> Result<DeviceHeartbeatResponse, DeviceError> {
        log::debug!(
            "Sending heartbeat for node {}: {:?}",
            node_id,
            metrics
        );

        let request = DeviceHeartbeatRequest {
            node_id: node_id.to_string(),
            metrics,
        };

        let response = self
            .client
            .post(format!("{}{}", self.base_url, API_NODES_HEARTBEAT))
            .header("Authorization", format!("Bearer {}", access_token))
            .json(&request)
            .send()
            .await
            .map_err(|e| DeviceError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DeviceError::HeartbeatError(format!(
                "HTTP {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| DeviceError::HeartbeatError(format!("解析响应失败: {}", e.to_string())))
    }

    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<DeviceRefreshResponse, DeviceError> {
        log::debug!("Refreshing access token");

        let response = self
            .client
            .post(format!("{}{}", self.base_url, API_NODES_REFRESH))
            .header("Authorization", format!("Bearer {}", refresh_token))
            .send()
            .await
            .map_err(|e| DeviceError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DeviceError::RefreshError(format!(
                "令牌刷新失败: {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| DeviceError::RefreshError(e.to_string()))
    }

    // 检查JWT令牌是否需要刷新（当剩余有效期小于指定阈值时）
    pub fn should_refresh_token(&self, token: &str, threshold_seconds: u64) -> Result<bool, DeviceError> {
        // 尝试获取过期时间
        let expiry = self.get_token_expiry(token)?;
        
        // 获取当前时间的秒数
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| DeviceError::TokenParseError(e.to_string()))?
            .as_secs();
        
        // 如果过期时间小于当前时间加上阈值，则应该刷新
        if expiry <= now + threshold_seconds {
            log::debug!("Token will expire soon (in {} seconds), should refresh", 
                        if expiry > now { expiry - now } else { 0 });
            Ok(true)
        } else {
            log::debug!("Token still valid for {} seconds", expiry - now);
            Ok(false)
        }
    }
    
    // 从JWT令牌中提取过期时间
    fn get_token_expiry(&self, token: &str) -> Result<u64, DeviceError> {
        // JWT格式: header.payload.signature
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(DeviceError::TokenParseError("无效的JWT格式".to_string()));
        }
        
        // 解码payload部分（Base64URL编码）
        let payload = general_purpose::URL_SAFE_NO_PAD.decode(parts[1])
            .map_err(|e| DeviceError::TokenParseError(format!("无法解码令牌: {}", e)))?;
        
        // 解析为JSON
        let payload_json: Value = serde_json::from_slice(&payload)
            .map_err(|e| DeviceError::TokenParseError(format!("无法解析令牌内容: {}", e)))?;
        
        // 提取exp字段（过期时间，Unix时间戳）
        let exp = payload_json["exp"].as_u64()
            .ok_or_else(|| DeviceError::TokenParseError("令牌中没有过期时间字段".to_string()))?;
        
        Ok(exp)
    }
}
