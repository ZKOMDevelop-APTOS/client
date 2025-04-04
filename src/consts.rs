// API 端点配置
pub const API_BASE_URL: &str = "https://zkom-backend.abo.network";
#[allow(dead_code)]
pub const API_VERSION: &str = "v1";

// 设备注册相关配置
#[allow(dead_code)]
pub const DEVICE_CODE_LENGTH: usize = 8;
#[allow(dead_code)]
pub const DEVICE_CODE_EXPIRY_SECONDS: u64 = 300; // 5分钟
pub const DEVICE_VERIFY_POLL_INTERVAL: u64 = 5; // 5秒

// API 路径
pub const API_NODES_INIT: &str = "/api/nodes/init";
pub const API_NODES_VERIFY: &str = "/api/nodes/verify";

// 配置相关
pub const CONFIG_DIR: &str = "zkom";
pub const CONFIG_FILE: &str = "config.json";

// 设备指纹相关
pub const FINGERPRINT_SEPARATOR: &str = ";";
pub const FINGERPRINT_CPU_PREFIX: &str = "CPU";
pub const FINGERPRINT_MEM_PREFIX: &str = "MEM";
pub const FINGERPRINT_OS_PREFIX: &str = "OS";

// 错误消息
pub const ERROR_DEVICE_INIT_FAILED: &str = "设备初始化失败";
pub const ERROR_DEVICE_VERIFY_FAILED: &str = "设备验证失败";
#[allow(dead_code)]
pub const ERROR_DEVICE_CODE_EXPIRED: &str = "设备码已过期";
#[allow(dead_code)]
pub const ERROR_DEVICE_DISABLED: &str = "设备已被禁用";
#[allow(dead_code)]
pub const ERROR_NETWORK: &str = "网络错误";

// 提示消息
pub const MSG_STARTING_NODE: &str = "Starting ZKOM node client...";
pub const MSG_NODE_CONFIGURED: &str = "Node already configured, starting...";
pub const MSG_DEVICE_VERIFY_SUCCESS: &str = "设备验证成功！";
pub const MSG_DEVICE_VERIFY_TIMEOUT: &str = "设备验证超时，请重新运行程序";
pub const MSG_NODE_STARTING: &str = "节点启动中...";
pub const MSG_NODE_ID: &str = "节点ID: {}";

// 验证提示
pub const MSG_VERIFY_INSTRUCTIONS: &str = "请访问以下网址完成设备验证：";
pub const MSG_VERIFY_URI: &str = "验证网址: {}";
pub const MSG_DEVICE_CODE: &str = "设备码: {}";
pub const MSG_CODE_EXPIRY: &str = "设备码有效期: {} 秒"; 