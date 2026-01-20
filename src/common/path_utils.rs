use std::path::Path;

/// Check if a path exists and is a file (helper to reduce redundancy)
pub fn is_valid_file(path: &Path) -> bool {
    path.exists() && path.is_file()
}

/// Check if a path exists and is a directory (helper to reduce redundancy)
pub fn is_valid_directory(path: &Path) -> bool {
    path.exists() && path.is_dir()
}
