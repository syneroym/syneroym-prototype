use crate::types::{FileInfo, StreamInfo};
use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

pub struct FileOperations;

impl FileOperations {
    pub fn validate_path(data_dir: &Path, path: &str) -> Result<PathBuf> {
        // Sanitize path
        if path.contains("..") || path.contains('/') || path.contains('\\') {
            return Err(anyhow!("Invalid path"));
        }

        let full_path = data_dir.join(path);

        // Ensure path is within data_dir
        if !full_path.starts_with(data_dir) {
            return Err(anyhow!("Path outside data directory"));
        }

        Ok(full_path)
    }

    pub async fn list_files(data_dir: &Path, prefix: Option<String>) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();

        let mut entries = tokio::fs::read_dir(data_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;

            if metadata.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();

                // Filter by prefix if provided
                if let Some(ref p) = prefix
                    && !name.starts_with(p)
                {
                    continue;
                }

                // Skip database files
                if name.ends_with(".db") {
                    continue;
                }

                let content_type = mime_guess::from_path(&name).first().map(|m| m.to_string());

                files.push(FileInfo {
                    name,
                    size: metadata.len(),
                    content_type,
                });
            }
        }

        Ok(files)
    }

    pub async fn read_small(data_dir: &Path, path: &str) -> Result<Vec<u8>> {
        let full_path = Self::validate_path(data_dir, path)?;
        let data = tokio::fs::read(&full_path).await?;
        Ok(data)
    }

    pub async fn write_small(data_dir: &Path, path: &str, data: Vec<u8>) -> Result<()> {
        let full_path = Self::validate_path(data_dir, path)?;
        tokio::fs::write(&full_path, data).await?;
        Ok(())
    }

    pub async fn delete(data_dir: &Path, path: &str) -> Result<bool> {
        let full_path = Self::validate_path(data_dir, path)?;

        if full_path.exists() {
            tokio::fs::remove_file(&full_path).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn exists(data_dir: &Path, path: &str) -> bool {
        if let Ok(full_path) = Self::validate_path(data_dir, path) {
            full_path.exists()
        } else {
            false
        }
    }

    pub async fn get_info(data_dir: &Path, path: &str) -> Result<FileInfo> {
        let full_path = Self::validate_path(data_dir, path)?;
        let metadata = tokio::fs::metadata(&full_path).await?;

        let content_type = mime_guess::from_path(path).first().map(|m| m.to_string());

        Ok(FileInfo {
            name: path.to_string(),
            size: metadata.len(),
            content_type,
        })
    }

    pub async fn open_read(data_dir: &Path, path: &str) -> Result<StreamInfo> {
        let full_path = Self::validate_path(data_dir, path)?;
        let metadata = tokio::fs::metadata(&full_path).await?;

        let content_type = mime_guess::from_path(path).first().map(|m| m.to_string());

        Ok(StreamInfo {
            id: String::new(), // Will be set by stream manager
            content_type,
            content_length: Some(metadata.len()),
            filename: Some(path.to_string()),
        })
    }
}
