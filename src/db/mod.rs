pub mod conversation_repo;
pub mod message_repo;
pub mod provider_repo;
pub mod schema;

use std::sync::Mutex;

use anyhow::Result;
use rusqlite::Connection;

/// 数据库连接包装，跨线程共享。
/// rusqlite::Connection 是 Send 但非 Sync，用 Mutex 包裹使其可在多线程间共享。
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 打开（或创建）SQLite 数据库文件，并执行迁移。
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(schema::SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 以独占方式获取连接，执行闭包。
    pub fn with_conn<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R>,
    {
        let conn = self.conn.lock().expect("db mutex poisoned");
        f(&conn)
    }
}
