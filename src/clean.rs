use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::safety;
use crate::targets;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trashed {
    pub path: PathBuf,
    pub bytes: u64,
    pub log_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashError {
    pub path: PathBuf,
    pub message: String,
}

/// Move one cache directory to the OS trash.
///
/// Refuses the scan root, symlinks, non-directories, unknown names, and
/// protected system paths. Does not permanently delete.
pub fn trash_one(path: &Path, root: &Path, bytes: u64) -> Result<Trashed, TrashError> {
    if let Err(message) = ensure_safe(path, root) {
        return Err(TrashError {
            path: path.to_path_buf(),
            message,
        });
    }

    match trash::delete(path) {
        Ok(()) => {
            let log_warning = append_log(path).err();
            Ok(Trashed {
                path: path.to_path_buf(),
                bytes,
                log_warning,
            })
        }
        Err(err) => Err(TrashError {
            path: path.to_path_buf(),
            message: err.to_string(),
        }),
    }
}

pub fn ensure_safe(path: &Path, root: &Path) -> Result<(), String> {
    if path == root {
        return Err("refusing to trash the scan root".to_string());
    }
    let Some(name) = path.file_name() else {
        return Err("path has no file name".to_string());
    };
    if targets::lookup(name).is_none() {
        return Err(format!(
            "refusing to trash {} because its name is not a known cache directory",
            path.display()
        ));
    }
    if safety::is_blocked(path) {
        return Err(format!("refusing to trash system path {}", path.display()));
    }

    let meta = fs::symlink_metadata(path).map_err(|err| err.to_string())?;
    if meta.file_type().is_symlink() {
        return Err("refusing to trash a symlink".to_string());
    }
    if !meta.is_dir() {
        return Err("refusing to trash a file; only cache directories are removed".to_string());
    }
    Ok(())
}

/// Append-only log of paths moved to the trash, so they can be restored by hand.
pub fn log_file() -> Result<PathBuf, String> {
    let dir = log_dir()?;
    fs::create_dir_all(&dir).map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    Ok(dir.join("moves.log"))
}

fn append_log(path: &Path) -> Result<(), String> {
    let file_path = log_file()?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .map_err(|err| format!("cannot open {}: {err}", file_path.display()))?;
    let stamp = crate::scan::format_timestamp(SystemTime::now());
    writeln!(file, "{stamp}\t{}", path.display())
        .map_err(|err| format!("cannot write {}: {err}", file_path.display()))?;
    Ok(())
}

fn log_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(local).join("devclean"));
        }
    }

    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "cannot locate the home directory for the restore log".to_string())?;

    #[cfg(target_os = "macos")]
    {
        Ok(home.join("Library/Logs/devclean"))
    }

    #[cfg(windows)]
    {
        Ok(home.join("AppData").join("Local").join("devclean"))
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Ok(home.join(".local/share/devclean"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn refuses_unknown_names_and_the_scan_root() {
        let dir = std::env::temp_dir().join(format!("devclean-clean-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("node_modules")).unwrap();

        let root_err = ensure_safe(&dir, &dir).unwrap_err();
        assert!(root_err.contains("scan root"), "{root_err}");

        let src_err = ensure_safe(&dir.join("src"), &dir).unwrap_err();
        assert!(src_err.contains("not a known cache"), "{src_err}");

        assert!(ensure_safe(&dir.join("node_modules"), &dir).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }
}
