mod config;
mod consts;
mod device;
mod runtime;

use anyhow::Result;
use chrono::{DateTime, Utc};
use config::ConfigManager;
use consts::*;
use device::{DeviceInfo, DeviceManager, GpuInfo, HardwareCollector, HardwareInfo};
use runtime::RuntimeChecker;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    env_logger::init();
    log::info!("{}", MSG_STARTING_NODE);

    // 检查运行时环境
    let runtime_checker = RuntimeChecker::new();
    runtime_checker.check_environment()?;

    // 初始化配置管理器
    let mut config_manager = ConfigManager::new()?;
    let config = config_manager.get_config();

    // 如果已经配置了访问令牌，直接启动节点
    if config.access_token.is_some() {
        log::info!("{}", MSG_NODE_CONFIGURED);
        return start_node(config_manager.get_config()).await;
    }

    // 收集设备信息
    let hardware_collector = HardwareCollector::new();
    let hardware_info = hardware_collector.collect_info()?;

    let cpu_serial = hardware_info.cpu_serial.clone();
    let gpu_uuid = hardware_info.gpu_uuid.clone();
    let system_fingerprint = hardware_info.system_fingerprint;
    let gpu_model = hardware_info.gpu_model;
    let gpu_memory = hardware_info.gpu_memory;
    let cuda_version = hardware_info.cuda_version;
    let driver_version = hardware_info.driver_version;

    // 初始化设备管理器
    let device_manager = DeviceManager::new(config.base_url.clone());

    // 创建设备信息
    let device_info = DeviceInfo {
        cpu_serial: cpu_serial.clone(),
        gpu_uuid: gpu_uuid.clone(),
        system_fingerprint,
        installation_hash: device_manager.generate_installation_hash(),
    };

    // 请求设备初始化
    let init_response = device_manager
        .init_device(
            device_info,
            GpuInfo {
                model: gpu_model.unwrap_or_else(|| "Unknown".to_string()),
                memory: gpu_memory.unwrap_or(0),
                cuda_version: cuda_version.unwrap_or_else(|| "Unknown".to_string()),
            },
            HardwareInfo {
                cpu_info: cpu_serial,
                gpu_did: gpu_uuid.unwrap_or_else(|| "Unknown".to_string()),
                driver_version: driver_version.unwrap_or_else(|| "Unknown".to_string()),
            },
        )
        .await?;

    // 保存设备码
    config_manager.set_device_code(init_response.device_code.clone())?;
    config_manager.set_user_code(init_response.user_code.clone())?;

    // 显示验证信息
    println!("{}", MSG_VERIFY_INSTRUCTIONS);
    println!(
        "{}",
        MSG_VERIFY_URI.replace("{}", &init_response.verification_uri)
    );
    println!(
        "{}",
        MSG_DEVICE_CODE.replace("{}", &init_response.device_code)
    );
    println!(
        "{}",
        MSG_CODE_EXPIRY.replace(
            "{}",
            &DateTime::parse_from_rfc3339(&init_response.expires_at)
                .unwrap()
                .with_timezone(&Utc)
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string()
        )
    );

    // 轮询验证状态
    let mut attempts = 0;
    let expires_at = DateTime::parse_from_rfc3339(&init_response.expires_at)
        .unwrap()
        .with_timezone(&Utc);
    let now = Utc::now();
    let max_attempts = (expires_at - now).num_seconds() / DEVICE_VERIFY_POLL_INTERVAL as i64;
    let user_code = init_response.user_code;

    while attempts < max_attempts {
        match device_manager.verify_device(&user_code).await {
            Ok(response) => {
                // 保存令牌和节点ID
                config_manager.set_tokens(response.access_token, response.refresh_token)?;
                config_manager.set_node_id(response.node_id.to_string())?;

                println!("{}", MSG_DEVICE_VERIFY_SUCCESS);
                break;
            }
            Err(e) => {
                log::warn!("{}", e);
                attempts += 1;
                tokio::time::sleep(Duration::from_secs(DEVICE_VERIFY_POLL_INTERVAL)).await;
            }
        }
    }

    if attempts >= max_attempts {
        println!("{}", MSG_DEVICE_VERIFY_TIMEOUT);
        return Ok(());
    }

    // 启动节点
    start_node(config_manager.get_config()).await
}

async fn start_node(config: &config::NodeConfig) -> Result<()> {
    // TODO: 实现节点启动逻辑
    println!("{}", MSG_NODE_STARTING);
    println!(
        "{}",
        MSG_NODE_ID.replace("{}", config.node_id.as_deref().unwrap_or("unknown"))
    );
    Ok(())
}
