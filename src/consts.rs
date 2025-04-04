// API 端点配置
pub const API_BASE_URL: &str = "https://zkom-backend.abo.network";
#[allow(dead_code)]
pub const API_VERSION: &str = "v1";

// NATS和Stable Diffusion配置
pub const NATS_SERVER_URL: &str = "nats://zkom-nats.abo.network:4222";
pub const SD_API_URL: &str = "http://localhost:7860";

// 设备注册相关配置
#[allow(dead_code)]
pub const DEVICE_CODE_LENGTH: usize = 8;
#[allow(dead_code)]
pub const DEVICE_CODE_EXPIRY_SECONDS: u64 = 300; // 5 minutes
pub const DEVICE_VERIFY_POLL_INTERVAL: u64 = 5; // 5 seconds

// 心跳相关配置
pub const HEARTBEAT_INTERVAL_SECONDS: u64 = 60; // Default 60 seconds heartbeat interval

// 令牌相关配置
pub const TOKEN_REFRESH_THRESHOLD_SECONDS: u64 = 300; // Refresh token when less than 5 minutes remaining

// API 路径
pub const API_NODES_INIT: &str = "/api/nodes/init";
pub const API_NODES_VERIFY: &str = "/api/nodes/verify";
pub const API_NODES_HEARTBEAT: &str = "/api/nodes/device/heartbeat";
pub const API_NODES_REFRESH: &str = "/api/nodes/device/refresh";

// 配置相关
pub const CONFIG_DIR: &str = "zkom";
pub const CONFIG_FILE: &str = "config.json";

// 设备指纹相关
pub const FINGERPRINT_SEPARATOR: &str = ";";
pub const FINGERPRINT_CPU_PREFIX: &str = "CPU";
pub const FINGERPRINT_MEM_PREFIX: &str = "MEM";
pub const FINGERPRINT_OS_PREFIX: &str = "OS";

// 错误消息
pub const ERROR_DEVICE_INIT_FAILED: &str = "Device initialization failed";
pub const ERROR_DEVICE_VERIFY_FAILED: &str = "Device verification failed";
#[allow(dead_code)]
pub const ERROR_DEVICE_CODE_EXPIRED: &str = "Device code expired";
#[allow(dead_code)]
pub const ERROR_DEVICE_DISABLED: &str = "Device has been disabled";
#[allow(dead_code)]
pub const ERROR_NETWORK: &str = "Network error";

// 提示消息
pub const MSG_STARTING_NODE: &str = "Starting ZKOM node client...";
pub const MSG_NODE_CONFIGURED: &str = "Node already configured, starting...";
pub const MSG_DEVICE_VERIFY_SUCCESS: &str = "Device verification successful!";
pub const MSG_DEVICE_VERIFY_TIMEOUT: &str = "Device verification timeout, please restart the program";
pub const MSG_NODE_STARTING: &str = "Node starting...";
pub const MSG_NODE_ID: &str = "Node ID: {}";

// 验证提示
pub const MSG_VERIFY_INSTRUCTIONS: &str = "Please visit the following URL to complete device verification:";
pub const MSG_VERIFY_URI: &str = "Verification URL: {}";
pub const MSG_DEVICE_CODE: &str = "Device Code: {}";
pub const MSG_CODE_EXPIRY: &str = "Code expiry: {} seconds"; 