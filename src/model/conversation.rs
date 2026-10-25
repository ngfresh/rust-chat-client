use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: i64,
    pub title: String,
    pub provider_id: i64,
    pub created_at: i64,
    pub updated_at: i64,
}
