use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::Role;

/// API 层错误类型。
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("API returned error: {0}")]
    Api(String),
}

/// 发送给 OpenAI 兼容接口的消息体。
#[derive(Debug, Serialize)]
pub struct ChatRequestMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
}

/// 请求体。
#[derive(Debug, Serialize)]
pub struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<ChatRequestMessage<'a>>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// 响应中的 choice。
#[derive(Debug, Deserialize)]
pub struct ChatResponseChoice {
    pub message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub role: Option<String>,
    pub content: Option<String>,
}

/// 响应体。
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<ChatResponseChoice>,
}

/// SSE 流式响应中的增量片段。
#[derive(Debug, Deserialize)]
pub struct StreamChunk {
    #[serde(default)]
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
pub struct StreamChoice {
    pub delta: StreamDelta,
}

#[derive(Debug, Deserialize)]
pub struct StreamDelta {
    pub content: Option<String>,
}

impl ChatRequest<'_> {
    /// 从领域消息构建请求消息列表。
    pub fn messages_from(messages: &[crate::model::Message]) -> Vec<ChatRequestMessage> {
        messages
            .iter()
            .map(|m| ChatRequestMessage {
                role: role_str(m.role),
                content: &m.content,
            })
            .collect()
    }
}

fn role_str(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    }
}
