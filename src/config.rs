//! 应用配置：从 exe 同目录下的 `config.toml` 读取 API Key 等。
//!
//! 这样双击 exe 即可生效，无需设置环境变量。
//! 配置文件示例（config.toml）：
//! ```toml
//! api_key  = "sk-你的DeepSeek_Key"
//! base_url = "https://api.deepseek.com"
//! model    = "deepseek-chat"
//! ```

use std::path::PathBuf;

use serde::Deserialize;

/// 应用配置。字段均可选，缺省时回退到数据库 / 内置默认。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

impl AppConfig {
    /// 配置文件所在路径：exe 同目录下的 config.toml。
    pub fn config_path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("config.toml")
    }

    /// 读取配置；文件不存在或解析失败时返回空配置（不 panic）。
    pub fn load() -> Self {
        let path = Self::config_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&text).unwrap_or_default()
    }

    /// 首次运行时生成一份模板配置，方便用户直接填写 Key。
    pub fn ensure_template() {
        let path = Self::config_path();
        if path.exists() {
            return;
        }
        let template = r#"# AI Chat 配置文件（与 exe 同目录）
# 填入你的大模型 API Key 即可，支持任何 OpenAI 兼容接口。

# API 密钥（必填）
api_key = ""

# 接口地址（OpenAI 兼容协议的 base url，不带结尾斜杠）
base_url = "https://api.deepseek.com"

# 模型名
model = "deepseek-chat"
"#;
        let _ = std::fs::write(&path, template);
    }
}
