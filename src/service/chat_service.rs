use std::sync::Arc;

use anyhow::Result;

use crate::api::LlmClient;
use crate::db::{conversation_repo, message_repo, provider_repo, Database};
use crate::model::Role;

/// 聊天编排服务：负责 DB 落库 + LLM 调用。
pub struct ChatService {
    db: Arc<Database>,
    llm: Arc<dyn LlmClient>,
}

impl ChatService {
    pub fn new(db: Arc<Database>, llm: Arc<dyn LlmClient>) -> Self {
        Self { db, llm }
    }

    /// 发送一条用户消息并获取 assistant 回复。
    /// Phase 1 使用非流式调用；返回完整的 assistant 文本。
    ///
    /// 注意：Phase 1 中 rusqlite 调用直接在 async 任务中同步执行。
    /// 本地 SQLite 操作是亚毫秒级，不会明显阻塞 UI；
    /// Phase 2 将迁移到 background_executor().spawn_blocking()。
    pub async fn send_message(&self, conversation_id: i64, user_content: &str) -> Result<String> {
        let now = chrono::Utc::now().timestamp();

        // 1. 保存用户消息
        self.db.with_conn(|conn| {
            message_repo::insert(conn, conversation_id, Role::User, user_content, now)
        })?;
        self.after_user_message(conversation_id, user_content, now)?;

        // 2. 加载历史消息
        let history = self.db.with_conn(|conn| {
            message_repo::list_by_conversation(conn, conversation_id)
        })?;

        // 3. 获取会话对应的 provider（并应用环境变量覆盖）
        let provider = self.db.with_conn(|conn| -> Result<_> {
            let conv = conversation_repo::get(conn, conversation_id)?
                .ok_or_else(|| anyhow::anyhow!("conversation not found: {}", conversation_id))?;
            let provider = provider_repo::get(conn, conv.provider_id)?
                .ok_or_else(|| anyhow::anyhow!("provider not found: {}", conv.provider_id))?;
            Ok(provider.with_env_overrides())
        })?;

        // 校验 API Key
        if provider.api_key.trim().is_empty() {
            anyhow::bail!(
                "API Key 未配置。请在 exe 同目录的 config.toml 中填入 api_key，\
                 或设置环境变量 AI_CHAT_API_KEY"
            );
        }

        // 4. 调用 LLM
        let reply = self
            .llm
            .chat(&provider, &history)
            .await
            .map_err(|e| anyhow::anyhow!("LLM error: {e}"))?;

        // 5. 保存 assistant 消息
        self.db.with_conn(|conn| {
            message_repo::insert(conn, conversation_id, Role::Assistant, &reply, now)
        })?;

        Ok(reply)
    }

    /// 流式发送：每收到一段增量文本调用一次 on_delta（在调用方所在的 executor 上执行）。
    /// 返回拼接后的完整 assistant 文本。
    ///
    /// on_delta 需要 `Send + 'static`：它会被移动到发起 HTTP 的后台任务里。
    pub async fn send_message_stream(
        &self,
        conversation_id: i64,
        user_content: &str,
        on_delta: impl FnMut(String) + Send + 'static,
    ) -> Result<String> {
        let now = chrono::Utc::now().timestamp();

        // 1. 保存用户消息
        self.db.with_conn(|conn| {
            message_repo::insert(conn, conversation_id, Role::User, user_content, now)
        })?;
        self.after_user_message(conversation_id, user_content, now)?;

        // 2. 加载历史消息
        let history = self
            .db
            .with_conn(|conn| message_repo::list_by_conversation(conn, conversation_id))?;

        // 3. 获取 provider（应用环境变量覆盖）
        let provider = self.db.with_conn(|conn| -> Result<_> {
            let conv = conversation_repo::get(conn, conversation_id)?
                .ok_or_else(|| anyhow::anyhow!("conversation not found: {}", conversation_id))?;
            let provider = provider_repo::get(conn, conv.provider_id)?
                .ok_or_else(|| anyhow::anyhow!("provider not found: {}", conv.provider_id))?;
            Ok(provider.with_env_overrides())
        })?;

        if provider.api_key.trim().is_empty() {
            anyhow::bail!(
                "API Key 未配置。请在 exe 同目录的 config.toml 中填入 api_key，\
                 或设置环境变量 AI_CHAT_API_KEY"
            );
        }

        // 4. 流式调用 LLM
        let reply = self
            .llm
            .chat_stream(&provider, &history, Box::new(on_delta))
            .await
            .map_err(|e| anyhow::anyhow!("LLM error: {e}"))?;

        // 5. 保存完整 assistant 消息
        self.db.with_conn(|conn| {
            message_repo::insert(conn, conversation_id, Role::Assistant, &reply, now)
        })?;

        Ok(reply)
    }

    /// 用户消息落库后的会话维护：
    /// 默认标题的会话用首条消息前 20 字自动命名；否则仅刷新 updated_at 使其排到列表顶部。
    fn after_user_message(&self, conversation_id: i64, user_content: &str, now: i64) -> Result<()> {
        self.db.with_conn(|conn| {
            if let Some(conv) = conversation_repo::get(conn, conversation_id)? {
                if conv.title.starts_with("新对话") {
                    let title: String = user_content.chars().take(20).collect();
                    conversation_repo::update_title(conn, conversation_id, &title, now)?;
                } else {
                    conversation_repo::touch(conn, conversation_id, now)?;
                }
            }
            Ok(())
        })
    }
}
