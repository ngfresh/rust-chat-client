use anyhow::Result;
use rusqlite::Connection;

use crate::model::Conversation;

/// 插入一个会话，返回新 id。
pub fn insert(conn: &Connection, title: &str, provider_id: i64, now: i64) -> Result<i64> {
    conn.execute(
        "INSERT INTO conversations (title, provider_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        rusqlite::params![title, provider_id, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 根据 id 获取会话。
pub fn get(conn: &Connection, id: i64) -> Result<Option<Conversation>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, provider_id, created_at, updated_at FROM conversations WHERE id = ?1",
    )?;
    let mut rows = stmt.query(rusqlite::params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(Conversation {
            id: row.get(0)?,
            title: row.get(1)?,
            provider_id: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        }))
    } else {
        Ok(None)
    }
}

/// 列出所有会话（按更新时间倒序）。
pub fn list_all(conn: &Connection) -> Result<Vec<Conversation>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, provider_id, created_at, updated_at FROM conversations ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Conversation {
            id: row.get(0)?,
            title: row.get(1)?,
            provider_id: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;
    let mut convs = Vec::new();
    for row in rows {
        convs.push(row?);
    }
    Ok(convs)
}

/// 更新会话标题与时间戳。
pub fn update_title(conn: &Connection, id: i64, title: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![title, now, id],
    )?;
    Ok(())
}

/// 仅刷新会话的 updated_at（用于新消息时让会话排到列表顶部）。
pub fn touch(conn: &Connection, id: i64, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )?;
    Ok(())
}

/// 删除会话及其所有消息（ON DELETE CASCADE）。
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM conversations WHERE id = ?1", rusqlite::params![id])?;
    Ok(())
}
