use super::*;
use rusqlite::params;

pub async fn execute(sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<usize> {
    let conn = get_db_connection().await;
    let conn = conn.lock().await;
    conn.execute(sql, params).context("Failed to execute SQL")
}

pub async fn query<T, F>(sql: &str, params: &[&dyn rusqlite::ToSql], f: F) -> Result<Vec<T>>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let conn = get_db_connection().await;
    let conn = conn.lock().await;
    let mut stmt = conn.prepare(sql).context("Failed to prepare statement")?;
    let rows = stmt.query_map(params, f).context("Failed to execute query")?;
    rows.collect::<rusqlite::Result<Vec<T>>>().context("Failed to collect query results")
}