use crate::utils::random_code;
use diesel::prelude::*;
use std::path::PathBuf;

#[derive(Debug)]
pub struct FileMetadata {
    pub code: String,
    pub file_name: String,
    pub file_size: usize,
    pub file_type: String,
    pub local_path: PathBuf,
}

impl FileMetadata {
    pub fn new(name: &str, file_size: usize, file_type: &str, local_path: PathBuf) -> Self {
        let code = random_code();
        Self {
            code,
            file_name: name.to_string(),
            file_size: file_size,
            file_type: file_type.to_string(),
            local_path: local_path,
        }
    }
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::file_metadata)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbFileMetadata {
    pub id: i32,
    pub code: String,
    pub file_name: String,
    pub file_size: i32,
    pub file_type: String,
    pub local_path: String,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::file_metadata)]
pub struct NewFileMetadata<'a> {
    pub code: &'a str,
    pub file_name: &'a str,
    pub file_size: i32,
    pub file_type: &'a str,
    pub local_path: &'a str,
    pub created_at: i32,
}

#[derive(AsChangeset)]
#[diesel(table_name = crate::schema::file_metadata)]
pub struct UpdateFileMetadata<'a> {
    pub file_name: Option<&'a str>,
    pub file_size: Option<i32>,
    pub file_type: Option<&'a str>,
    pub local_path: Option<&'a str>,
    pub updated_at: i32,
}

impl<'a> From<&'a DbFileMetadata> for NewFileMetadata<'a> {
    fn from(file_metadata: &'a DbFileMetadata) -> Self {
        NewFileMetadata {
            code: &file_metadata.code,
            file_name: &file_metadata.file_name,
            file_size: file_metadata.file_size,
            file_type: &file_metadata.file_type,
            local_path: &file_metadata.local_path,
            created_at: file_metadata.created_at,
        }
    }
}
