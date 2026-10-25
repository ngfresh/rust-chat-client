use anyhow::Result;
use rusqlite::Connection;

use crate::model::{Message, Role};

/// 插入一条消息，返回新 id。
pub fn insert(conn: &Connection, conversation_id: i64, role: Role, content: &str, now: i64) -> Result<i64> {
    conn.execute(
        "INSERT INTO messages (conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![conversation_id, role.as_str(), content, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 列出某会话的所有消息（按 id 升序）。
pub fn list_by_conversation(conn: &Connection, conversation_id: i64) -> Result<Vec<Message>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, created_at FROM messages WHERE conversation_id = ?1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(rusqlite::params![conversation_id], |row| {
        let role_str: String = row.get(2)?;
        Ok(Message {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            role: Role::from_str(&role_str),
            content: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }
    Ok(messages)
}

/// 删除某条消息。
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM messages WHERE id = ?1", rusqlite::params![id])?;
    Ok(())
}
