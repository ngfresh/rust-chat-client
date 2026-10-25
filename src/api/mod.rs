pub mod openai_compatible;
pub mod runtime;
pub mod types;

use async_trait::async_trait;

use crate::model::{Message, Provider};
use crate::api::types::ApiError;

/// 大模型客户端统一接口。
/// 新增厂商（Anthropic / Gemini 等）只需实现此 trait。
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// 非流式聊天，返回完整回复文本。
    async fn chat(&self, provider: &Provider, messages: &[Message]) -> Result<String, ApiError>;

    /// 流式聊天；每收到一段增量文本就调用一次 on_delta。
    /// 返回拼接后的完整回复文本。
    ///
    /// 回调用 `Box<dyn FnMut>` 而非泛型，以保持 trait 对象安全
    ///（ChatService 中以 `Arc<dyn LlmClient>` 持有）。
    async fn chat_stream(
        &self,
        provider: &Provider,
        messages: &[Message],
        on_delta: Box<dyn FnMut(String) + Send>,
    ) -> Result<String, ApiError>;
}
