use crate::models::file_metadata::{DbFileMetadata, NewFileMetadata};
use crate::schema::file_metadata;
use anyhow::{Context, Result};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

pub fn create_file_metadata(
    conn: &mut SqliteConnection,
    file_metadata: &NewFileMetadata,
) -> Result<()> {
    diesel::insert_into(file_metadata::table)
        .values(file_metadata)
        .execute(conn)
        .context("Failed to create file metadata")?;
    Ok(())
}

pub fn get_local_path(conn: &mut SqliteConnection, code: &str) -> Result<String> {
    file_metadata::table
        .filter(file_metadata::code.eq(code))
        .select(file_metadata::local_path)
        .first(conn)
        .context("Failed to get local path")
}

pub fn get_file_metadata(conn: &mut SqliteConnection, code: &str) -> Result<Option<DbFileMetadata>> {
    file_metadata::table
        .filter(file_metadata::code.eq(code))
        .first(conn)
        .optional()
        .context("Failed to get file metadata")
}

pub fn delete_file_metadata(conn: &mut SqliteConnection, code: &str) -> Result<()> {
    diesel::delete(file_metadata::table.filter(file_metadata::code.eq(code)))
        .execute(conn)
        .context("Failed to delete file metadata")?;
    Ok(())
}

pub fn delete_all_file_metadata(conn: &mut SqliteConnection) -> Result<()> {
    diesel::delete(file_metadata::table)
        .execute(conn)
        .context("Failed to delete all file metadata")?;
    Ok(())
}

mod tests {
    use super::*;
    use crate::db::tests::setup_test_db;

    #[ctor::ctor]
    fn setup() {}

    #[ctor::dtor]
    fn teardown() {
        let mut conn = setup_test_db();
        delete_all_file_metadata(&mut conn).unwrap();
    }

    #[test]
    fn test_() {
        let mut conn = setup_test_db();
        let file_metadata = NewFileMetadata {
            code: "test_code",
            file_name: "test_file_name",
            file_size: 100,
            file_type: "test_file_type",
            local_path: "test_local_path",
            created_at: 0,
        };
        create_file_metadata(&mut conn, &file_metadata).unwrap();

        // 测试 get_file_metadata
        let file_metadata = get_file_metadata(&mut conn, "test_code").unwrap().unwrap();
        assert_eq!(file_metadata.code, "test_code");
        assert_eq!(file_metadata.file_name, "test_file_name");
        assert_eq!(file_metadata.file_size, 100);
        assert_eq!(file_metadata.file_type, "test_file_type");
        assert_eq!(file_metadata.local_path, "test_local_path");

        // 测试 get_local_path
        let local_path = get_local_path(&mut conn, "test_code").unwrap();
        assert_eq!(local_path, "test_local_path");

        // 测试 delete_file_metadata
        delete_file_metadata(&mut conn, "test_code").unwrap();
        let result = get_file_metadata(&mut conn, "test_code").unwrap();
        assert!(result.is_none());
    }
}
