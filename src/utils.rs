//! 工具函数模块
//!
//! 提供 CDB 可执行文件查找、Windows 注册表访问和文件搜索等实用功能。

use std::path::{Path, PathBuf};

/// 转储文件信息
#[derive(Debug, Clone)]
pub struct DumpFileInfo {
    /// 文件路径
    pub path: PathBuf,
    /// 文件大小（字节）
    pub size_bytes: u64,
}

/// 查找 CDB 可执行文件
///
/// 如果提供了自定义路径，则验证该路径是否存在。
/// 否则，在默认位置列表中搜索 CDB。
///
/// # 参数
/// * `custom_path` - 可选的自定义 CDB 路径
///
/// # 返回
/// 如果找到 CDB，返回其路径；否则返回 None
pub fn find_cdb_executable(custom_path: Option<&Path>) -> Option<PathBuf> {
    // 如果提供了自定义路径，检查是否存在
    if let Some(path) = custom_path {
        if path.exists() && path.is_file() {
            return Some(path.to_path_buf());
        }
        return None;
    }

    // 默认 CDB 路径列表
    const DEFAULT_CDB_PATHS: &[&str] = &[
        // Windows 11/10 SDK (x64)
        r"C:\Program Files (x86)\Windows Kits\10\Debuggers\x64\cdb.exe",
        // Windows 11/10 SDK (x86)
        r"C:\Program Files (x86)\Windows Kits\10\Debuggers\x86\cdb.exe",
        // Windows 11/10 SDK (arm64)
        r"C:\Program Files (x86)\Windows Kits\10\Debuggers\arm64\cdb.exe",
        // Windows 8.1 SDK (x64)
        r"C:\Program Files (x86)\Windows Kits\8.1\Debuggers\x64\cdb.exe",
        // Windows 8.1 SDK (x86)
        r"C:\Program Files (x86)\Windows Kits\8.1\Debuggers\x86\cdb.exe",
        // WinDbg Preview from Microsoft Store (x64)
        r"C:\Program Files\WindowsApps\Microsoft.WinDbg_1.2306.14001.0_x64__8wekyb3d8bbwe\cdb.exe",
        // 旧版本路径
        r"C:\Program Files\Debugging Tools for Windows (x64)\cdb.exe",
        r"C:\Program Files (x86)\Debugging Tools for Windows (x86)\cdb.exe",
    ];

    // 在默认路径中搜索
    for path_str in DEFAULT_CDB_PATHS {
        let path = PathBuf::from(path_str);
        if path.exists() && path.is_file() {
            return Some(path);
        }
    }

    // 尝试在 PATH 环境变量中查找
    if let Ok(path_env) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_env) {
            let cdb_path = dir.join("cdb.exe");
            if cdb_path.exists() && cdb_path.is_file() {
                return Some(cdb_path);
            }
        }
    }

    None
}

/// 从 Windows 注册表读取本地转储路径
///
/// 读取系统配置的崩溃转储目录路径。
///
/// # 返回
/// 如果找到配置的转储路径，返回该路径；否则返回 None
#[cfg(windows)]
pub fn get_local_dumps_path() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    // 尝试读取 LocalDumps 注册表项
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // 常见的转储路径注册表位置
    let dump_paths = [
        r"SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps",
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\AeDebug",
    ];

    for reg_path in &dump_paths {
        if let Ok(key) = hklm.open_subkey(reg_path) {
            // 尝试读取 DumpFolder 值
            if let Ok(dump_folder) = key.get_value::<String, _>("DumpFolder") {
                let path = PathBuf::from(dump_folder);
                if path.exists() && path.is_dir() {
                    return Some(path);
                }
            }
        }
    }

    // 如果注册表中没有配置，返回默认的 Windows 转储目录
    let default_paths = [
        r"C:\Windows\Minidump",
        r"C:\ProgramData\Microsoft\Windows\WER\ReportQueue",
        r"C:\Users\Public\Documents\Dumps",
    ];

    for path_str in &default_paths {
        let path = PathBuf::from(path_str);
        if path.exists() && path.is_dir() {
            return Some(path);
        }
    }

    None
}

/// 非 Windows 平台的占位实现
#[cfg(not(windows))]
pub fn get_local_dumps_path() -> Option<PathBuf> {
    None
}

/// 在目录中搜索转储文件
///
/// 搜索指定目录中的 .dmp 文件。
///
/// # 参数
/// * `directory` - 要搜索的目录路径
/// * `recursive` - 是否递归搜索子目录
///
/// # 返回
/// 返回找到的转储文件列表，包含路径和大小信息
///
/// # 错误
/// 如果目录不存在或无法读取，返回 I/O 错误
pub fn find_dump_files(
    directory: &Path,
    recursive: bool,
) -> Result<Vec<DumpFileInfo>, std::io::Error> {
    let mut dump_files = Vec::new();

    if !directory.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("目录不存在: {}", directory.display()),
        ));
    }

    if !directory.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("路径不是目录: {}", directory.display()),
        ));
    }

    search_directory(directory, recursive, &mut dump_files)?;

    // 按文件大小降序排序（大文件通常更重要）
    dump_files.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    Ok(dump_files)
}

/// 递归搜索目录中的转储文件（内部辅助函数）
fn search_directory(
    directory: &Path,
    recursive: bool,
    dump_files: &mut Vec<DumpFileInfo>,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            // 检查文件扩展名是否为 .dmp
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("dmp") {
                    // 获取文件大小
                    if let Ok(metadata) = entry.metadata() {
                        dump_files.push(DumpFileInfo {
                            path: path.clone(),
                            size_bytes: metadata.len(),
                        });
                    }
                }
            }
        } else if recursive && path.is_dir() {
            // 递归搜索子目录
            search_directory(&path, recursive, dump_files)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_cdb_executable_custom_path() {
        // 测试不存在的自定义路径
        let result = find_cdb_executable(Some(Path::new("nonexistent.exe")));
        assert!(result.is_none());
    }

    #[test]
    fn test_find_cdb_executable_default() {
        // 这个测试在没有安装 CDB 的系统上会失败，所以只检查返回类型
        let result = find_cdb_executable(None);
        // 如果找到了 CDB，路径应该以 cdb.exe 结尾
        if let Some(path) = result {
            assert!(path.to_string_lossy().to_lowercase().ends_with("cdb.exe"));
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_get_local_dumps_path() {
        // 这个测试只检查函数是否能正常运行
        let result = get_local_dumps_path();
        // 如果找到了路径，应该是一个目录
        if let Some(path) = result {
            assert!(path.is_dir());
        }
    }

    #[test]
    fn test_find_dump_files_nonexistent_directory() {
        let result = find_dump_files(Path::new("nonexistent_dir"), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_dump_files_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = find_dump_files(temp_dir.path(), false).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_find_dump_files_with_dumps() {
        let temp_dir = TempDir::new().unwrap();

        // 创建一些测试文件
        let dump1 = temp_dir.path().join("test1.dmp");
        let dump2 = temp_dir.path().join("test2.DMP"); // 测试大小写不敏感
        let not_dump = temp_dir.path().join("test.txt");

        fs::write(&dump1, b"dummy dump 1").unwrap();
        fs::write(&dump2, b"dummy dump 2 larger").unwrap();
        fs::write(&not_dump, b"not a dump").unwrap();

        let result = find_dump_files(temp_dir.path(), false).unwrap();

        // 应该找到 2 个 .dmp 文件
        assert_eq!(result.len(), 2);

        // 验证文件按大小降序排序
        assert!(result[0].size_bytes >= result[1].size_bytes);

        // 验证路径正确
        let paths: Vec<_> = result.iter().map(|f| &f.path).collect();
        assert!(paths.contains(&&dump1));
        assert!(paths.contains(&&dump2));
    }

    #[test]
    fn test_find_dump_files_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        // 在根目录创建一个转储文件
        let dump1 = temp_dir.path().join("root.dmp");
        fs::write(&dump1, b"root dump").unwrap();

        // 在子目录创建一个转储文件
        let dump2 = sub_dir.join("sub.dmp");
        fs::write(&dump2, b"sub dump").unwrap();

        // 非递归搜索应该只找到根目录的文件
        let result = find_dump_files(temp_dir.path(), false).unwrap();
        assert_eq!(result.len(), 1);

        // 递归搜索应该找到两个文件
        let result = find_dump_files(temp_dir.path(), true).unwrap();
        assert_eq!(result.len(), 2);
    }
}
