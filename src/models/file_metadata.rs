use std::path::Path;


#[derive(Debug)]
pub struct FileMetadata {
    pub code: String,
    pub file_name: String,
    pub file_size: usize,
    pub file_type: String,
    pub local_path: Path,
}

impl FileMetadata {
    pub fn new() {
        // 随机生成 file code
        let code;

        Self {
            code
        }
    }
}


