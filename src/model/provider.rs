use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub default_model: String,
}

impl Provider {
    /// 应用运行时覆盖。优先级：环境变量 > 配置文件(config.toml) > 数据库。
    ///
    /// 配置文件位于 exe 同目录，双击 exe 即可生效，无需设置环境变量；
    /// 环境变量仍保留最高优先级，便于开发调试。
    pub fn with_env_overrides(mut self) -> Self {
        // 1. 配置文件
        let cfg = crate::config::AppConfig::load();
        if let Some(v) = cfg.api_key.filter(|s| !s.trim().is_empty()) {
            self.api_key = v;
        }
        if let Some(v) = cfg.base_url.filter(|s| !s.trim().is_empty()) {
            self.base_url = v;
        }
        if let Some(v) = cfg.model.filter(|s| !s.trim().is_empty()) {
            self.default_model = v;
        }

        // 2. 环境变量（最高优先级）
        if let Ok(v) = std::env::var("AI_CHAT_API_KEY") {
            if !v.trim().is_empty() {
                self.api_key = v;
            }
        }
        if let Ok(v) = std::env::var("AI_CHAT_BASE_URL") {
            if !v.trim().is_empty() {
                self.base_url = v;
            }
        }
        if let Ok(v) = std::env::var("AI_CHAT_MODEL") {
            if !v.trim().is_empty() {
                self.default_model = v;
            }
        }
        self
    }
}
