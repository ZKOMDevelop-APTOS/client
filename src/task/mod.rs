use anyhow::Result;
use async_nats::{self, Client, Message};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::time::Instant;
use crate::stable_diffusion::{StableDiffusion, SDConfig, TextToImageParams};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// 任务消息结构
#[derive(Debug, Clone, Deserialize)]
pub struct TaskMessage {
    pub task_id: String,
    pub node_id: String,
    pub params: serde_json::Value,
}

/// 任务结果结构
#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    pub task_id: String,
    pub status: String,
    pub duration_sec: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_urls: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_stack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub retries: u32,
}

/// 任务处理器配置
#[derive(Debug, Clone)]
pub struct TaskProcessorConfig {
    pub nats_server: String,
    pub sd_url: String,
    pub node_id: String,
}

/// 任务处理器
pub struct TaskProcessor {
    config: TaskProcessorConfig,
    nats_client: Client,
    sd: StableDiffusion,
}

impl TaskProcessor {
    /// 创建新的任务处理器
    pub async fn new(config: TaskProcessorConfig) -> Result<Self> {
        // 连接到NATS服务器
        log::debug!("Connecting to NATS server: {}", config.nats_server);
        let nats_client = async_nats::connect(&config.nats_server).await?;
        log::info!("Connected to NATS server: {}", config.nats_server);
        log::debug!("NATS connection details: {:?}", nats_client);
        
        // 创建Stable Diffusion客户端
        let sd_config = SDConfig {
            base_url: config.sd_url.clone(),
            timeout: Some(120000), // 默认超时时间2分钟
        };
        
        let sd = StableDiffusion::new(sd_config)?;
        
        Ok(Self {
            config,
            nats_client,
            sd,
        })
    }
    
    /// 开始处理任务
    pub async fn start_processing(&self) -> Result<()> {
        // 获取JetStream上下文
        log::debug!("Getting JetStream context");
        let jetstream = async_nats::jetstream::new(self.nats_client.clone());
        
        // 订阅JetStream流
        log::debug!("Subscribing to 'TASKS' stream using JetStream");
        let stream = jetstream.get_stream("TASKS").await?;
        let consumer = stream.get_or_create_consumer("zkom-processor", async_nats::jetstream::consumer::pull::Config::default()).await?;
        let mut messages = consumer.messages().await?;
        log::info!("Subscribed to 'TASKS' stream using JetStream");
        
        log::info!("Starting task processing loop");
        // 处理接收到的任务
        'main_loop: loop {
            while let Some(msg) = messages.next().await {
                match msg {
                    Ok(msg) => {
                        log::debug!("Received JetStream message from subject: {}", msg.subject);
                        log::debug!("JetStream message headers: {:?}", msg.headers);
                        log::debug!("JetStream message payload size: {} bytes", msg.payload.len());
                        
                        // 输出消息内容的前100个字符作为调试信息 (或者全部内容如果少于100字符)
                        let preview = String::from_utf8_lossy(&msg.payload);
                        let preview_len = std::cmp::min(preview.len(), 100);
                        log::debug!("JetStream message preview: {}{}", 
                            &preview[..preview_len], 
                            if preview.len() > 100 { "..." } else { "" }
                        );
                        
                        // 记录消息处理开始
                        log::debug!("Starting to process JetStream message");
                        let nats_msg = msg.message.clone(); // 克隆消息以避免部分移动
                        if let Err(e) = self.process_task(nats_msg).await {
                            log::error!("Error processing task: {:?}", e);
                        }
                        
                        // 确认消息已处理
                        if let Err(e) = msg.ack().await {
                            log::error!("Failed to acknowledge message: {:?}", e);
                        }
                    },
                    Err(e) => {
                        log::error!("Error receiving JetStream message: {:?}", e);
                        
                        // 检查是否为连接相关错误
                        log::warn!("Connection error detected, attempting to reconnect: {:?}", e);
                        break; // 退出内部循环，尝试重新连接
                    }
                }
            }
            
            // 如果内部循环结束，表示连接可能已断开，尝试重新连接
            log::warn!("JetStream subscription interrupted, attempting to reconnect in 5 seconds");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            
            // 重新连接
            let retry_count = 3;
            let mut reconnected = false;
            
            for attempt in 1..=retry_count {
                log::info!("Reconnection attempt {}/{}", attempt, retry_count);
                
                match async_nats::jetstream::new(self.nats_client.clone()).get_stream("TASKS").await {
                    Ok(stream) => {
                        match stream.get_or_create_consumer("zkom-processor", async_nats::jetstream::consumer::pull::Config::default()).await {
                            Ok(consumer) => {
                                match consumer.messages().await {
                                    Ok(new_messages) => {
                                        messages = new_messages;
                                        reconnected = true;
                                        log::info!("Successfully reconnected to JetStream");
                                        break;
                                    },
                                    Err(e) => log::error!("Failed to get messages after reconnection: {:?}", e)
                                }
                            },
                            Err(e) => log::error!("Failed to get consumer after reconnection: {:?}", e)
                        }
                    },
                    Err(e) => log::error!("Failed to get stream after reconnection: {:?}", e)
                }
                
                // 指数退避重试
                let backoff = std::time::Duration::from_secs(2u64.pow(attempt));
                log::info!("Waiting {}s before next reconnection attempt", backoff.as_secs());
                tokio::time::sleep(backoff).await;
            }
            
            // 如果重连失败，则退出主循环
            if !reconnected {
                log::error!("Failed to reconnect after {} attempts, exiting task processing", retry_count);
                break 'main_loop;
            }
            
            log::info!("Resuming task processing loop");
        }
        
        log::warn!("JetStream subscription ended");
        Ok(())
    }
    
    /// 处理单个任务
    async fn process_task(&self, msg: Message) -> Result<()> {
        let start_time = Instant::now();
        
        // 尝试解析任务消息
        match serde_json::from_slice::<TaskMessage>(&msg.payload) {
            Ok(task_message) => {
                let task_id = task_message.task_id.clone(); // 克隆任务ID以便后续使用
                log::info!("Received task: {}", task_id);
                log::debug!("Task message details: {:?}", task_message);
                
                if task_message.node_id != self.config.node_id {
                    log::warn!("Invalid node ID for task {}", task_id);
                    
                    // 发送失败结果
                    let result = TaskResult {
                        task_id: task_message.task_id,
                        status: "failed".to_string(),
                        duration_sec: 0.0,
                        result_urls: None,
                        error_stack: Some("Invalid node ID".to_string()),
                        node_id: Some(self.config.node_id.clone()),
                        retries: 0,
                    };
                    
                    self.publish_result(&result).await?;
                    return Ok(());
                }
                
                // 执行任务
                log::info!("Processing task: {}", task_id);
                log::info!("Task params: {:?}", task_message.params);
                
                match self.execute_task(&task_message).await {
                    Ok(result_urls) => {
                        // 计算处理时间
                        let duration = start_time.elapsed().as_secs_f64();
                        
                        // 构建成功结果
                        let result = TaskResult {
                            task_id: task_message.task_id.clone(),
                            status: "completed".to_string(),
                            duration_sec: 3.0,
                            result_urls: Some(result_urls),
                            error_stack: None,
                            node_id: Some(self.config.node_id.clone()),
                            retries: 0,
                        };
                        
                        // 发布结果
                        log::debug!("Publishing task result for task {}: {:?}", task_id, result);
                        self.publish_result(&result).await?;
                        log::info!("Task {} completed in {:.2}s", task_id, duration);
                    },
                    Err(e) => {
                        // 计算处理时间
                        let duration = start_time.elapsed().as_secs_f64();
                        
                        // 构建错误结果
                        let result = TaskResult {
                            task_id: task_message.task_id,
                            status: "failed".to_string(),
                            duration_sec: duration,
                            result_urls: None,
                            error_stack: Some(format!("{:?}", e)),
                            node_id: Some(self.config.node_id.clone()),
                            retries: 0,
                        };
                        
                        // 发布结果
                        log::debug!("Publishing error result for task {}: {:?}", task_id, result);
                        self.publish_result(&result).await?;
                        log::error!("Task {} failed: {:?}", task_id, e);
                    }
                }
            },
            Err(e) => {
                log::error!("Failed to parse task message: {:?}", e);
                log::debug!("Raw NATS message payload: {:?}", String::from_utf8_lossy(&msg.payload));
                
                // 构建错误结果，使用新的UUID作为任务ID
                let result = TaskResult {
                    task_id: Uuid::new_v4().to_string(),
                    status: "failed".to_string(),
                    duration_sec: 0.0,
                    result_urls: None,
                    error_stack: Some(format!("Failed to parse task message: {:?}", e)),
                    node_id: Some(self.config.node_id.clone()),
                    retries: 0,
                };
                
                // 发布结果
                log::debug!("Publishing parse error result: {:?}", result);
                self.publish_result(&result).await?;
            }
        }
        
        Ok(())
    }
    
    /// 执行具体任务
    async fn execute_task(&self, task: &TaskMessage) -> Result<Vec<String>> {
        // 从参数中提取提示词
        let prompt = match task.params.get("prompt") {
            Some(p) => p.as_str().ok_or_else(|| anyhow::anyhow!("Prompt must be a string"))?.to_string(),
            None => return Err(anyhow::anyhow!("Missing required parameter: prompt")),
        };
        
        // 从参数中提取其他可选参数
        let width = task.params.get("width")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
            
        let height = task.params.get("height")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
            
        let steps = task.params.get("steps")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
            
        let cfg_scale = task.params.get("cfg_scale")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32);
            
        let seed = task.params.get("seed")
            .and_then(|v| v.as_i64());
            
        let negative_prompt = task.params.get("negative_prompt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        // 创建SD参数
        let params = TextToImageParams {
            prompt,
            negative_prompt,
            width,
            height,
            steps,
            cfg_scale,
            seed,
        };
        
        // 调用SD API生成图像
        let result = self.sd.text_to_image(params).await?;
        
        // 将base64图像转换为URL格式
        let image_urls = result.images
            .iter()
            .map(|img| StableDiffusion::base64_to_image_url(img))
            .collect();
            
        Ok(image_urls)
    }
    
    /// 发布任务结果到NATS
    async fn publish_result(&self, result: &TaskResult) -> Result<()> {
        let payload = serde_json::to_string(result)?;
        log::debug!("Publishing result to 'results' subject, payload size: {} bytes", payload.len());
        
        // 输出完整的结果内容用于调试
        log::debug!("Task result details: task_id={}, status={}, duration={}s", 
            result.task_id, result.status, result.duration_sec);
        
        if let Some(urls) = &result.result_urls {
            log::debug!("Task generated {} images", urls.len());
            for (i, url) in urls.iter().enumerate() {
                let preview = if url.len() > 50 { 
                    format!("{}...", &url[..50]) 
                } else { 
                    url.clone() 
                };
                log::debug!("  Image {}: {}", i + 1, preview);
            }
        }
        
        if let Some(error) = &result.error_stack {
            log::debug!("Task error details: {}", error);
        }
        
        // 获取JetStream上下文
        log::debug!("Getting JetStream context for publishing result");
        let jetstream = async_nats::jetstream::new(self.nats_client.clone());
        
        // 使用JetStream发布结果
        let subject = format!("results.{}", result.task_id);
        log::debug!("Publishing result to '{}' subject", subject);
        jetstream.publish(subject.clone(), payload.into()).await?;
        log::debug!("Result published successfully to '{}' subject using JetStream", subject);
        Ok(())
    }
} 