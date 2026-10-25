use anyhow::Result;
use rusqlite::Connection;

use crate::model::Provider;

/// 插入一个 Provider，返回新 id。
pub fn insert(conn: &Connection, name: &str, base_url: &str, api_key: &str, default_model: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO providers (name, base_url, api_key, default_model) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, base_url, api_key, default_model],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 根据 id 获取 Provider。
pub fn get(conn: &Connection, id: i64) -> Result<Option<Provider>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, base_url, api_key, default_model FROM providers WHERE id = ?1",
    )?;
    let mut rows = stmt.query(rusqlite::params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(Provider {
            id: row.get(0)?,
            name: row.get(1)?,
            base_url: row.get(2)?,
            api_key: row.get(3)?,
            default_model: row.get(4)?,
        }))
    } else {
        Ok(None)
    }
}

/// 获取第一个 Provider（Phase 1 硬编码使用）。
pub fn get_first(conn: &Connection) -> Result<Option<Provider>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, base_url, api_key, default_model FROM providers ORDER BY id ASC LIMIT 1",
    )?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        Ok(Some(Provider {
            id: row.get(0)?,
            name: row.get(1)?,
            base_url: row.get(2)?,
            api_key: row.get(3)?,
            default_model: row.get(4)?,
        }))
    } else {
        Ok(None)
    }
}

/// 列出所有 Provider。
pub fn list_all(conn: &Connection) -> Result<Vec<Provider>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, base_url, api_key, default_model FROM providers ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Provider {
            id: row.get(0)?,
            name: row.get(1)?,
            base_url: row.get(2)?,
            api_key: row.get(3)?,
            default_model: row.get(4)?,
        })
    })?;
    let mut providers = Vec::new();
    for row in rows {
        providers.push(row?);
    }
    Ok(providers)
}
