use std::path::Path;

use walkdir::WalkDir;

/// Total size of regular files under `path`. Symlinks are not followed.
///
/// Unreadable entries are skipped so one locked file does not hide the rest.
pub fn folder_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.metadata().map(|meta| meta.len()).unwrap_or(0))
        .sum()
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn sums_files_and_ignores_symlink_targets() {
        let dir = std::env::temp_dir().join(format!("devclean-size-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("nested")).unwrap();
        fs::write(dir.join("a.txt"), vec![1; 10]).unwrap();
        fs::write(dir.join("nested/b.txt"), vec![2; 25]).unwrap();

        let outside =
            std::env::temp_dir().join(format!("devclean-size-out-{}", std::process::id()));
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("big.bin"), vec![3; 1000]).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.join("big.bin"), dir.join("link.bin")).unwrap();

        assert_eq!(folder_size(&dir), 35);

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn formats_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }
}
