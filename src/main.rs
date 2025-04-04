mod config;
mod consts;
mod device;
mod runtime;
mod stable_diffusion;
mod task;

use anyhow::Result;
use chrono::{DateTime, Utc};
use config::ConfigManager;
use consts::*;
use device::{DeviceInfo, DeviceManager, DeviceMetrics, GpuInfo, HardwareCollector, HardwareInfo};
use runtime::RuntimeChecker;
use std::time::Duration;
use task::{TaskProcessor, TaskProcessorConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    if std::env::var("RUST_LOG").is_err() {
        // 使用 env_logger::Builder 而不是直接设置环境变量
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Info)
            .filter_module("zkom_client", log::LevelFilter::Debug)
            .filter_module("async_nats", log::LevelFilter::Debug)
            .init();
    } else {
        // 如果已设置 RUST_LOG，使用默认初始化
        env_logger::init();
    }
    
    log::info!("{}", MSG_STARTING_NODE);
    log::debug!("NATS server URL: {}", NATS_SERVER_URL);

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
    let system_fingerprint = hardware_info.system_fingerprint.clone();
    let gpu_model = hardware_info.gpu_model.clone();
    let gpu_memory = hardware_info.gpu_memory;
    let cuda_version = hardware_info.cuda_version.clone();
    let driver_version = hardware_info.driver_version.clone();

    // 初始化设备管理器
    let device_manager = DeviceManager::new(config.base_url.clone());

    // 创建设备信息
    let device_info = DeviceInfo {
        cpu_serial: cpu_serial.clone(),
        gpu_uuid: gpu_uuid.clone(),
        system_fingerprint: system_fingerprint.clone(),
        installation_hash: device_manager.generate_installation_hash(),
    };

    // 请求设备初始化
    let init_response = device_manager
        .init_device(
            device_info,
            GpuInfo {
                model: gpu_model.as_ref().unwrap_or(&"Unknown".to_string()).to_string(),
                memory: gpu_memory.unwrap_or(0),
                cuda_version: cuda_version.as_ref().unwrap_or(&"Unknown".to_string()).to_string(),
            },
            HardwareInfo {
                cpu_serial,
                gpu_uuid,
                system_fingerprint,
                gpu_model,
                gpu_memory,
                cuda_version,
                driver_version,
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
    // 确保节点已配置
    let node_id = match &config.node_id {
        Some(id) => id.clone(),
        None => {
            log::error!("节点未配置，请先初始化节点");
            return Err(anyhow::anyhow!("节点未配置"));
        }
    };

    let access_token = match &config.access_token {
        Some(token) => token.clone(),
        None => {
            log::error!("访问令牌未配置，请先初始化节点");
            return Err(anyhow::anyhow!("访问令牌未配置"));
        }
    };
    
    let refresh_token = match &config.refresh_token {
        Some(token) => token.clone(),
        None => {
            log::error!("刷新令牌未配置，请先初始化节点");
            return Err(anyhow::anyhow!("刷新令牌未配置"));
        }
    };
    
    // 克隆base_url以便在异步闭包中使用
    let base_url = config.base_url.clone();

    println!("{}", MSG_NODE_STARTING);
    println!("{}", MSG_NODE_ID.replace("{}", &node_id));

    // 初始化设备管理器
    let device_manager = DeviceManager::new(base_url.clone());
    
    // 初始化硬件信息收集器
    let hardware_collector = HardwareCollector::new();
    
    // 启动任务处理器
    let task_config = TaskProcessorConfig {
        nats_server: std::env::var("NATS_SERVER").unwrap_or_else(|_| NATS_SERVER_URL.to_string()),
        sd_url: std::env::var("SD_URL").unwrap_or_else(|_| SD_API_URL.to_string()),
        node_id: node_id.clone(),
    };
    
    // 输出 NATS 相关配置信息
    log::info!("NATS configuration:");
    log::info!("  Server URL: {}", task_config.nats_server);
    log::info!("  Node ID: {}", task_config.node_id);
    log::info!("  Subjects: tasks (subscribe), task_results (publish)");
    
    // 创建任务处理器
    log::info!("Initializing task processor with NATS server: {}", task_config.nats_server);
    let task_processor = TaskProcessor::new(task_config).await?;
    
    // 启动心跳和任务处理
    let heartbeat_handle = tokio::spawn(async move {
        // 设置心跳间隔（秒）
        let heartbeat_interval = HEARTBEAT_INTERVAL_SECONDS;
        
        log::info!("Starting heartbeat reporting, interval: {} seconds", heartbeat_interval);
        
        // 初始化访问令牌
        let mut current_access_token = access_token;
        let current_refresh_token = refresh_token;
        
        // 创建配置管理器，处理错误而不是传播
        let mut config_manager = match ConfigManager::new() {
            Ok(cm) => cm,
            Err(e) => {
                log::error!("Failed to create config manager: {}", e);
                return;
            }
        };
        
        loop {
            // 检查令牌是否即将过期，如果是，则刷新
            if let Ok(should_refresh) = device_manager.should_refresh_token(&current_access_token, TOKEN_REFRESH_THRESHOLD_SECONDS) {
                if should_refresh {
                    log::info!("Access token about to expire, starting active refresh");
                    
                    // 尝试刷新令牌
                    match device_manager.refresh_token(&current_refresh_token).await {
                        Ok(refresh_response) => {
                            log::info!("Token refresh successful");
                            
                            // 更新当前使用的令牌
                            current_access_token = refresh_response.access_token.clone();
                            
                            // 保存新的访问令牌到配置
                            if let Err(save_err) = config_manager.update_access_token(refresh_response.access_token) {
                                log::error!("Failed to save new access token: {}", save_err);
                            }
                        }
                        Err(refresh_err) => {
                            log::error!("Active token refresh failed: {}", refresh_err);
                        }
                    }
                }
            } else {
                log::warn!("Unable to parse token expiry, will refresh on 401 error");
            }
            
            // 收集GPU指标
            match hardware_collector.collect_gpu_metrics() {
                Ok(gpu_metrics) => {
                    // 转换为设备指标
                    let device_metrics = DeviceMetrics {
                        gpu_utilization: gpu_metrics.utilization,
                        gpu_memory_used: gpu_metrics.memory_used,
                        gpu_temperature: gpu_metrics.temperature,
                        timestamp: gpu_metrics.timestamp,
                    };
                    
                    // 发送心跳
                match device_manager.send_heartbeat(&node_id, device_metrics, &current_access_token).await {
                    Ok(response) => {
                        log::debug!("Heartbeat sent successfully: {}", response.message);
                    }
                    Err(e) => {
                        // 检查是否是授权错误 (假设401状态码导致了特定的错误信息)
                        if e.to_string().contains("401") {
                            log::warn!("Access token expired, attempting to refresh token");
                            
                            // 尝试刷新令牌
                            match device_manager.refresh_token(&current_refresh_token).await {
                                Ok(refresh_response) => {
                                    log::info!("Token refresh successful");
                                    
                                    // 更新当前使用的令牌
                                    current_access_token = refresh_response.access_token.clone();
                                    
                                    // 保存新的访问令牌到配置
                                    if let Err(save_err) = config_manager.update_access_token(refresh_response.access_token) {
                                        log::error!("Failed to save new access token: {}", save_err);
                                    }
                                }
                                Err(refresh_err) => {
                                    log::error!("Token refresh failed: {}", refresh_err);
                                }
                            }
                            } else {
                                log::error!("Failed to send heartbeat: {}", e);
                        }
                    }
                    }
                }
                Err(e) => {
                    log::error!("Failed to collect GPU metrics: {}", e);
                }
            }
            
            // 等待下一次心跳
            tokio::time::sleep(Duration::from_secs(heartbeat_interval)).await;
        }
    });
    
    // 启动任务处理
    let task_handle = tokio::spawn(async move {
        log::info!("Starting NATS task processor");
        if let Err(e) = task_processor.start_processing().await {
            log::error!("Task processor error: {:?}", e);
            log::debug!("NATS task processor error details: {:?}", e);
        }
    });
    
    log::info!("NATS task processor and heartbeat services started");
    
    // 等待任务结束
    let _ = tokio::try_join!(
        async { 
            heartbeat_handle.await.map_err(|e| anyhow::anyhow!("Heartbeat processing error: {:?}", e)) 
        },
        async { 
            task_handle.await.map_err(|e| anyhow::anyhow!("Task processing error: {:?}", e)) 
        }
    );
    
    Ok(())
}
