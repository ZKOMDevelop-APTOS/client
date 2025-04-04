use anyhow::Result;
use reqwest::{Client, ClientBuilder, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for Stable Diffusion API client
#[derive(Debug, Clone)]
pub struct SDConfig {
    /// Base URL for the Stable Diffusion API
    pub base_url: String,
    /// Timeout in milliseconds (defaults to 120000 - 2 minutes)
    pub timeout: Option<u64>,
}

/// Parameters for text-to-image generation
#[derive(Debug, Clone, Serialize)]
pub struct TextToImageParams {
    /// Main prompt describing what to generate
    pub prompt: String,
    /// Negative prompt describing what to avoid
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// Width of the generated image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Height of the generated image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Number of sampling steps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
    /// Classifier free guidance scale
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cfg_scale: Option<f32>,
    /// Random seed (-1 for random)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
}

/// Response from the image generation API
#[derive(Debug, Clone, Deserialize)]
pub struct ImageResponse {
    /// Array of base64-encoded images
    pub images: Vec<String>,
    /// Parameters used for generation
    pub parameters: serde_json::Value,
    /// Additional information
    pub info: String,
}

/// Client for interacting with Stable Diffusion API
#[derive(Debug, Clone)]
pub struct StableDiffusion {
    client: Client,
    config: SDConfig,
}

impl StableDiffusion {
    /// Create a new Stable Diffusion client
    pub fn new(config: SDConfig) -> Result<Self> {
        let timeout = Duration::from_millis(config.timeout.unwrap_or(120000));
        
        let client = ClientBuilder::new()
            .timeout(timeout)
            .build()?;
            
        Ok(Self { client, config })
    }
    
    /// Generate images from text prompts
    pub async fn text_to_image(&self, params: TextToImageParams) -> Result<ImageResponse> {
        // 最大重试次数
        const MAX_RETRIES: u32 = 5;
        // 初始重试延迟（毫秒）
        const INITIAL_RETRY_DELAY_MS: u64 = 1000;
        
        // Create the request parameters with defaults
        let request_params = serde_json::json!({
            "prompt": params.prompt,
            "negative_prompt": params.negative_prompt.unwrap_or_else(|| "".to_string()),
            "width": params.width.unwrap_or(512),
            "height": params.height.unwrap_or(512),
            "steps": params.steps.unwrap_or(20),
            "cfg_scale": params.cfg_scale.unwrap_or(7.0),
            "seed": params.seed.unwrap_or(-1),
            "batch_size": 1,
            "n_iter": 1,
            "restore_faces": false,
            "tiling": false,
        });
        
        log::debug!("Sending request to Stable Diffusion API with params: {}", 
            serde_json::to_string_pretty(&request_params).unwrap_or_else(|_| format!("{:?}", request_params)));
        
        // Build the endpoint URL
        let url = Url::parse(&format!("{}/sdapi/v1/txt2img", self.config.base_url))?;
        
        // 重试逻辑
        let mut last_error = None;
        
        for retry in 0..MAX_RETRIES {
            if retry > 0 {
                // 指数退避延迟
                let delay = INITIAL_RETRY_DELAY_MS * 2u64.pow(retry - 1);
                log::warn!("Retrying Stable Diffusion API request (attempt {}/{}), waiting {}ms before retry", 
                    retry + 1, MAX_RETRIES, delay);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            
            // Send the request
            match self.client.post(url.clone())
                .header("Content-Type", "application/json")
                .json(&request_params)
                .send()
                .await 
            {
                Ok(response) => {
                    // Handle non-successful status codes
                    if !response.status().is_success() {
                        let status = response.status();
                        let error_text = response.text().await?;
                        
                        log::error!("Stable Diffusion API error: HTTP {}: {}", status, error_text);
                        
                        // 检查是否为服务器错误（可能是临时性故障）
                        let retry_error = error_text.contains("'NoneType' object") || 
                                         error_text.contains("CUDA out of memory") ||
                                         error_text.contains("expected scalar type") ||
                                         status.is_server_error();
                                         
                        if retry_error && retry < MAX_RETRIES - 1 {
                            log::warn!("Retryable error detected: HTTP {}: {}", status, error_text);
                            last_error = Some(anyhow::anyhow!("Stable Diffusion API request failed: HTTP {}: {}", status, error_text));
                            continue; // 继续重试
                        }
                        
                        return Err(anyhow::anyhow!("Stable Diffusion API request failed: HTTP {}: {}", status, error_text));
                    }
                    
                    // 获取响应内容
                    let response_text = response.text().await?;
                    
                    // 尝试解析JSON
                    match serde_json::from_str::<ImageResponse>(&response_text) {
                        Ok(image_response) => {
                            if image_response.images.is_empty() && retry < MAX_RETRIES - 1 {
                                log::warn!("Stable Diffusion API returned empty images array, retrying...");
                                last_error = Some(anyhow::anyhow!("Stable Diffusion API returned empty images array"));
                                continue; // 继续重试
                            }
                            
                            // 记录成功响应
                            log::debug!("Successfully received {} images from Stable Diffusion API",
                                image_response.images.len());
                                
                            return Ok(image_response);
                        },
                        Err(e) => {
                            // 日志记录响应内容的前 200 个字符
                            let preview_len = std::cmp::min(response_text.len(), 200);
                            log::error!("Failed to parse response JSON: {}, response preview: {}{}",
                                e, 
                                &response_text[..preview_len],
                                if response_text.len() > 200 { "..." } else { "" });
                                
                            if retry < MAX_RETRIES - 1 {
                                log::warn!("Failed to parse Stable Diffusion API response: {}, retrying...", e);
                                last_error = Some(anyhow::anyhow!("Failed to parse response: {}", e));
                                continue; // 继续重试
                            }
                            
                            return Err(anyhow::anyhow!("Failed to parse response: {}", e));
                        }
                    }
                },
                Err(e) => {
                    if retry < MAX_RETRIES - 1 {
                        log::warn!("Stable Diffusion API request failed: {}, retrying...", e);
                        last_error = Some(anyhow::anyhow!("Request failed: {}", e));
                        continue; // 继续重试
                    }
                    return Err(anyhow::anyhow!("Request failed: {}", e));
                }
            }
        }
        
        // 如果所有重试都失败，返回最后一个错误
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed to connect to Stable Diffusion API after {} retries", MAX_RETRIES)))
    }
    
    /// Convert base64 image data to a data URL
    pub fn base64_to_image_url(base64_data: &str) -> String {
        format!("data:image/png;base64,{}", base64_data)
    }
} 