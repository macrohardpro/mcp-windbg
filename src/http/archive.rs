use flate2::read::GzDecoder;
use serde::Serialize;
use std::fs::File;
use std::io::{self};
use std::path::{Path, PathBuf};
use tar::Archive;
use tracing::{debug};
use zip::ZipArchive;

use crate::http::error::{HttpError, HttpResult};

/// Archive format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
    TarGz,
}

/// Extraction limits
const MAX_FILES: usize = 10_000;
const MAX_TOTAL_SIZE: u64 = 2 * 1024 * 1024 * 1024; // 2GB

/// Archive extractor
pub struct Extractor;

impl Extractor {
    /// Detect archive format from file extension
    pub fn detect_format(path: &Path) -> HttpResult<ArchiveFormat> {
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or_else(|| HttpError::UnsupportedFormat("No file extension".to_string()))?
            .to_lowercase();
        
        match extension.as_str() {
            "zip" => Ok(ArchiveFormat::Zip),
            "7z" => Ok(ArchiveFormat::SevenZ),
            "gz" | "tgz" => {
                // Check if it's a tar.gz
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if stem.ends_with(".tar") {
                        return Ok(ArchiveFormat::TarGz);
                    }
                }
                Ok(ArchiveFormat::TarGz)
            }
            _ => Err(HttpError::UnsupportedFormat(format!(
                "Unsupported extension: {}",
                extension
            ))),
        }
    }
    
    /// Extract archive to destination directory
    pub async fn extract(archive_path: &Path, dest_dir: &Path) -> HttpResult<()> {
        let format = Self::detect_format(archive_path)?;
        
        debug!("Extracting {:?} archive: {:?}", format, archive_path);
        
        // Run extraction in blocking task to avoid blocking async runtime
        let archive_path = archive_path.to_path_buf();
        let dest_dir = dest_dir.to_path_buf();
        
        tokio::task::spawn_blocking(move || match format {
            ArchiveFormat::Zip => Self::extract_zip(&archive_path, &dest_dir),
            ArchiveFormat::SevenZ => Self::extract_7z(&archive_path, &dest_dir),
            ArchiveFormat::TarGz => Self::extract_tar_gz(&archive_path, &dest_dir),
        })
        .await
        .map_err(|e| HttpError::ExtractionFailed(format!("Task join error: {}", e)))??;
        
        Ok(())
    }
    
    /// Extract ZIP archive
    fn extract_zip(archive_path: &Path, dest_dir: &Path) -> HttpResult<()> {
        let file = File::open(archive_path)
            .map_err(|e| HttpError::ExtractionFailed(format!("Failed to open ZIP: {}", e)))?;
        
        let mut archive = ZipArchive::new(file)
            .map_err(|e| HttpError::ExtractionFailed(format!("Failed to read ZIP: {}", e)))?;
        
        if archive.len() > MAX_FILES {
            return Err(HttpError::ExtractionFailed(format!(
                "Too many files in archive: {} (max: {})",
                archive.len(),
                MAX_FILES
            )));
        }
        
        let mut total_size = 0u64;
        
        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| HttpError::ExtractionFailed(format!("Failed to read file {}: {}", i, e)))?;
            
            let file_path = file.name().to_string();
            
            // Sanitize path
            let safe_path = Self::sanitize_path(&file_path)?;
            let out_path = dest_dir.join(&safe_path);
            
            // Check total size
            total_size += file.size();
            if total_size > MAX_TOTAL_SIZE {
                return Err(HttpError::ExtractionFailed(format!(
                    "Total extracted size exceeds limit: {} bytes (max: {} bytes)",
                    total_size, MAX_TOTAL_SIZE
                )));
            }
            
            if file.is_dir() {
                std::fs::create_dir_all(&out_path)
                    .map_err(|e| HttpError::ExtractionFailed(format!("Failed to create directory: {}", e)))?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| HttpError::ExtractionFailed(format!("Failed to create parent directory: {}", e)))?;
                }
                
                let mut out_file = File::create(&out_path)
                    .map_err(|e| HttpError::ExtractionFailed(format!("Failed to create file: {}", e)))?;
                
                io::copy(&mut file, &mut out_file)
                    .map_err(|e| HttpError::ExtractionFailed(format!("Failed to write file: {}", e)))?;
            }
        }
        
        debug!("Extracted {} files from ZIP", archive.len());
        Ok(())
    }
    
    /// Extract 7z archive
    fn extract_7z(archive_path: &Path, dest_dir: &Path) -> HttpResult<()> {
        sevenz_rust::decompress_file(archive_path, dest_dir)
            .map_err(|e| HttpError::ExtractionFailed(format!("Failed to extract 7z: {}", e)))?;
        
        debug!("Extracted 7z archive");
        Ok(())
    }
    
    /// Extract tar.gz archive
    fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> HttpResult<()> {
        let file = File::open(archive_path)
            .map_err(|e| HttpError::ExtractionFailed(format!("Failed to open tar.gz: {}", e)))?;
        
        let gz = GzDecoder::new(file);
        let mut archive = Archive::new(gz);
        
        let mut file_count = 0;
        let mut total_size = 0u64;
        
        for entry in archive.entries()
            .map_err(|e| HttpError::ExtractionFailed(format!("Failed to read tar entries: {}", e)))? {
            
            let mut entry = entry
                .map_err(|e| HttpError::ExtractionFailed(format!("Failed to read tar entry: {}", e)))?;
            
            file_count += 1;
            if file_count > MAX_FILES {
                return Err(HttpError::ExtractionFailed(format!(
                    "Too many files in archive: {} (max: {})",
                    file_count, MAX_FILES
                )));
            }
            
            let path = entry.path()
                .map_err(|e| HttpError::ExtractionFailed(format!("Failed to read entry path: {}", e)))?;
            
            let path_str = path.to_string_lossy().to_string();
            
            // Sanitize path
            let safe_path = Self::sanitize_path(&path_str)?;
            let out_path = dest_dir.join(&safe_path);
            
            // Check total size
            total_size += entry.size();
            if total_size > MAX_TOTAL_SIZE {
                return Err(HttpError::ExtractionFailed(format!(
                    "Total extracted size exceeds limit: {} bytes (max: {} bytes)",
                    total_size, MAX_TOTAL_SIZE
                )));
            }
            
            entry.unpack(&out_path)
                .map_err(|e| HttpError::ExtractionFailed(format!("Failed to unpack entry: {}", e)))?;
        }
        
        debug!("Extracted {} files from tar.gz", file_count);
        Ok(())
    }
    
    /// Sanitize file path to prevent path traversal attacks
    fn sanitize_path(path: &str) -> HttpResult<PathBuf> {
        let path = path.replace('\\', "/");
        
        // Check for absolute paths
        if path.starts_with('/') || path.contains(':') {
            return Err(HttpError::PathTraversal);
        }
        
        // Check for parent directory references
        if path.contains("..") {
            return Err(HttpError::PathTraversal);
        }
        
        // Build safe path
        let safe_path = PathBuf::from(&path);
        
        // Additional validation: ensure the path doesn't escape
        for component in safe_path.components() {
            match component {
                std::path::Component::Normal(_) => {}
                std::path::Component::RootDir | std::path::Component::ParentDir => {
                    return Err(HttpError::PathTraversal);
                }
                _ => {}
            }
        }
        
        Ok(safe_path)
    }
}

/// Extracted files information
#[derive(Debug, Default)]
pub struct ExtractedFiles {
    pub dump_files: Vec<PathBuf>,
    pub symbol_dirs: Vec<PathBuf>,
    pub source_dirs: Vec<PathBuf>,
}

/// Scan extracted files to identify dump files, symbol directories, and source directories
pub async fn scan_extracted_files(dir: &Path) -> HttpResult<ExtractedFiles> {
    let dir = dir.to_path_buf();
    
    tokio::task::spawn_blocking(move || {
        let mut result = ExtractedFiles::default();
        
        scan_directory(&dir, &mut result)?;
        
        debug!(
            "Scan results: {} dump files, {} symbol dirs, {} source dirs",
            result.dump_files.len(),
            result.symbol_dirs.len(),
            result.source_dirs.len()
        );
        
        Ok(result)
    })
    .await
    .map_err(|e| HttpError::Internal(format!("Task join error: {}", e)))?
}

fn scan_directory(dir: &Path, result: &mut ExtractedFiles) -> HttpResult<()> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| HttpError::Io(e))?;
    
    for entry in entries {
        let entry = entry.map_err(|e| HttpError::Io(e))?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                match ext.to_lowercase().as_str() {
                    "dmp" => {
                        debug!("Found dump file: {:?}", path);
                        result.dump_files.push(path);
                    }
                    _ => {}
                }
            }
        } else if path.is_dir() {
            // Check if directory contains PDB files (symbol directory)
            if contains_files_with_extension(&path, "pdb") {
                debug!("Found symbol directory: {:?}", path);
                result.symbol_dirs.push(path.clone());
            }
            
            // Check if directory contains source files
            if contains_source_files(&path) {
                debug!("Found source directory: {:?}", path);
                result.source_dirs.push(path.clone());
            }
            
            // Recursively scan subdirectories
            scan_directory(&path, result)?;
        }
    }
    
    Ok(())
}

fn contains_files_with_extension(dir: &Path, extension: &str) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                        if ext.eq_ignore_ascii_case(extension) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn contains_source_files(dir: &Path) -> bool {
    const SOURCE_EXTENSIONS: &[&str] = &["c", "cpp", "cc", "cxx", "h", "hpp", "hxx", "rs", "go", "java", "cs"];
    
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                        let ext_lower = ext.to_lowercase();
                        if SOURCE_EXTENSIONS.contains(&ext_lower.as_str()) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// File entry for tree display
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub file_type: String,
}

/// Build a flat file list with metadata for frontend display
pub fn build_file_tree(dir: &Path) -> HttpResult<Vec<FileEntry>> {
    let mut entries = Vec::new();
    walk_for_tree(dir, dir, &mut entries)?;
    Ok(entries)
}

fn walk_for_tree(base: &Path, dir: &Path, entries: &mut Vec<FileEntry>) -> HttpResult<()> {
    let read_dir = std::fs::read_dir(dir).map_err(|e| HttpError::Io(e))?;
    for entry in read_dir {
        let entry = entry.map_err(|e| HttpError::Io(e))?;
        let path = entry.path();
        let name = path.strip_prefix(base)
            .unwrap_or(&path)
            .display()
            .to_string();

        if path.is_file() {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let file_type = classify_file(&path);
            entries.push(FileEntry { name, size, file_type });
        } else if path.is_dir() {
            entries.push(FileEntry { name: format!("{}/", name), size: 0, file_type: "dir".to_string() });
            walk_for_tree(base, &path, entries)?;
        }
    }
    Ok(())
}

fn classify_file(path: &Path) -> String {
    match path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).as_deref() {
        Some("dmp") => "dump".to_string(),
        Some("pdb") => "symbol".to_string(),
        Some("c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "hxx" | "rs" | "go" | "java" | "cs") => "source".to_string(),
        _ => "other".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_format() {
        assert_eq!(
            Extractor::detect_format(Path::new("test.zip")).unwrap(),
            ArchiveFormat::Zip
        );
        assert_eq!(
            Extractor::detect_format(Path::new("test.7z")).unwrap(),
            ArchiveFormat::SevenZ
        );
        assert_eq!(
            Extractor::detect_format(Path::new("test.tar.gz")).unwrap(),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            Extractor::detect_format(Path::new("test.tgz")).unwrap(),
            ArchiveFormat::TarGz
        );
        
        assert!(Extractor::detect_format(Path::new("test.txt")).is_err());
    }
    
    #[test]
    fn test_sanitize_path() {
        // Valid paths
        assert!(Extractor::sanitize_path("file.txt").is_ok());
        assert!(Extractor::sanitize_path("dir/file.txt").is_ok());
        assert!(Extractor::sanitize_path("dir/subdir/file.txt").is_ok());
        
        // Invalid paths
        assert!(Extractor::sanitize_path("../file.txt").is_err());
        assert!(Extractor::sanitize_path("/etc/passwd").is_err());
        assert!(Extractor::sanitize_path("C:\\Windows\\System32").is_err());
        assert!(Extractor::sanitize_path("dir/../../../etc/passwd").is_err());
    }
}
