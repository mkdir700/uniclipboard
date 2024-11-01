use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest_dav::list_cmd::ListFile;
use serde::{Deserialize, Serialize};

use crate::{models::FileMetadata, Payload};

pub struct FileMetadataManager;

impl FileMetadataManager {
    pub fn add(&self, metadata: &FileMetadata) -> Result<()> {
        todo!()
    }

    pub fn add_from_payload(&self, payload: &Payload) -> Result<()> {
        todo!()
    }

    pub fn get(&self, file_code: &str) -> Result<FileMetadata> {
        todo!()
    }

    pub fn list(&self) -> Result<Vec<FileMetadata>> {
        todo!()
    }
}
