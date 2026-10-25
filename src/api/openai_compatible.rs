use async_trait::async_trait;
use futures::StreamExt;

use crate::api::runtime::runtime;
use crate::api::types::{ApiError, ChatRequest, ChatResponse, StreamChunk};
use crate::api::LlmClient;
use crate::model::{Message, Provider};

/// OpenAI 兼容协议客户端。
/// 适用于 OpenAI、DeepSeek、Moonshot、Ollama（OpenAI 模式）、vLLM 等。
///
/// 注意：所有 reqwest 调用都在进程级 Tokio runtime（`api::runtime`）上执行，
/// 因为 GPUI 的 executor 不提供 Tokio reactor，直接运行会 panic。
pub struct OpenAiClient {
    http: reqwest::Client,
}

impl OpenAiClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }
}

impl Default for OpenAiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn chat(&self, provider: &Provider, messages: &[Message]) -> Result<String, ApiError> {
        let http = self.http.clone();
        let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
        let key = provider.api_key.clone();
        let body = serde_json::json!({
            "model": provider.default_model,
            "messages": ChatRequest::messages_from(messages),
            "stream": false,
        });

        let join = runtime().spawn(async move {
            let resp = http.post(&url).bearer_auth(&key).json(&body).send().await?;
            let status = resp.status();
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                return Err(ApiError::Api(format!("HTTP {}: {}", status, text)));
            }
            let chat_resp: ChatResponse = resp.json().await?;
            Ok(chat_resp
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message.content)
                .unwrap_or_default())
        });

        // JoinHandle 的轮询不依赖 reactor，可在 GPUI executor 上安全 await
        join.await
            .map_err(|e| ApiError::Api(format!("network task join error: {e}")))?
    }

    async fn chat_stream(
        &self,
        provider: &Provider,
        messages: &[Message],
        mut on_delta: Box<dyn FnMut(String) + Send>,
    ) -> Result<String, ApiError> {
        let http = self.http.clone();
        let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
        let key = provider.api_key.clone();
        let body = serde_json::json!({
            "model": provider.default_model,
            "messages": ChatRequest::messages_from(messages),
            "stream": true,
        });

        let join = runtime().spawn(async move {
            let resp = http.post(&url).bearer_auth(&key).json(&body).send().await?;
            let status = resp.status();
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                return Err(ApiError::Api(format!("HTTP {}: {}", status, text)));
            }

            let mut full = String::new();
            // SSE 的行可能被 TCP 切分到不同 chunk，用 buffer 累积完整行再解析。
            let mut buffer = String::new();
            let mut byte_stream = resp.bytes_stream();

            while let Some(chunk) = byte_stream.next().await {
                let chunk = chunk?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim_end_matches(['\r', '\n']).to_string();
                    buffer.drain(..=pos);

                    let Some(data) = line.strip_prefix("data:") else {
                        continue;
                    };
                    let data = data.trim();
                    if data == "[DONE]" {
                        continue;
                    }

                    if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                        if let Some(content) = chunk
                            .choices
                            .first()
                            .and_then(|c| c.delta.content.clone())
                        {
                            if !content.is_empty() {
                                full.push_str(&content);
                                on_delta(content);
                            }
                        }
                    }
                }
            }
            Ok(full)
        });

        join.await
            .map_err(|e| ApiError::Api(format!("network task join error: {e}")))?
    }
}
