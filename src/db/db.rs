use rusqlite::{Connection, Result as SqliteResult};
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{error, info};
use anyhow::{Result, Context};
use once_cell::sync::Lazy;

pub struct DbManager {
    conn: Arc<Mutex<Connection>>,
}

pub static DB_MANAGER: Lazy<DbManager> = Lazy::new(|| {
    DbManager::new("devices.db").expect("Failed to create DbManager")
});

impl DbManager {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open database at {}", db_path))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn get_connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    pub async fn initialize(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        self.create_tables(&conn).context("Failed to create tables")?;
        Ok(())
    }

    fn create_tables(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS devices (
                id TEXT PRIMARY KEY,
                ip TEXT,
                port INTEGER,
                server_port INTEGER
            )",
            [],
        ).context("Failed to create devices table")?;

        // 在这里添加其他表的创建语句
        // 例如：
        // conn.execute(
        //     "CREATE TABLE IF NOT EXISTS other_table (...)",
        //     [],
        // ).context("Failed to create other_table")?;

        Ok(())
    }

    pub async fn execute_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock().await;
        let tx = conn.transaction().context("Failed to start transaction")?;
        
        match f(&tx) {
            Ok(result) => {
                tx.commit().context("Failed to commit transaction")?;
                Ok(result)
            }
            Err(e) => {
                tx.rollback().context("Failed to rollback transaction")?;
                Err(e)
            }
        }
    }

    pub async fn migrate(&self) -> Result<()> {
        // 在这里实现数据库迁移逻辑
        // 例如，可以使用一个版本表来跟踪数据库架构的版本
        // 然后根据当前版本执行必要的迁移脚本
        Ok(())
    }
}

// 提供一个便捷函数来获取数据库连接
pub async fn get_db_connection() -> Arc<Mutex<Connection>> {
    DB_MANAGER.get_connection().await
}

// 初始化数据库
pub async fn initialize_database() -> Result<()> {
    DB_MANAGER.initialize().await.context("Failed to initialize database")?;
    DB_MANAGER.migrate().await.context("Failed to migrate database")?;
    Ok(())
}

// 数据库操作的辅助函数
pub mod helpers {
    use super::*;

}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_initialization() {
        let db = DbManager::new(":memory:").expect("Failed to create in-memory database");
        db.initialize().await.expect("Failed to initialize database");
        
        // 验证表是否被创建
        let conn = db.get_connection().await;
        let conn = conn.lock().await;
        let result: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='devices'",
            [],
            |row| row.get(0),
        ).expect("Failed to query table existence");
        
        assert_eq!(result, 1, "Devices table should exist");
    }

    // 添加更多测试...
}