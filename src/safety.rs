use std::path::{Component, Path, PathBuf};

use crate::targets;

/// Short OS name shown in the TUI header.
pub fn os_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        std::env::consts::OS
    }
}

/// Canonicalize `path` and refuse filesystem roots and OS directories.
pub fn validate_root(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!(
            "path does not exist: {}. Pass a project folder.",
            path.display()
        ));
    }

    let root = path.canonicalize().map_err(|err| {
        format!(
            "cannot read {}: {err}. Pass a project folder.",
            path.display()
        )
    })?;

    if !root.is_dir() {
        return Err(format!(
            "{} is not a directory. Pass a project folder.",
            root.display()
        ));
    }

    if is_filesystem_root(&root) {
        return Err("refusing to scan the filesystem root. Pass a project folder.".to_string());
    }

    if is_blocked(&root) {
        return Err(format!(
            "refusing to scan system path {}. Pass a project folder.",
            root.display()
        ));
    }

    if root.file_name().and_then(targets::lookup).is_some() {
        return Err(format!(
            "refusing to scan cache folder {} itself. Open the project that contains it.",
            root.display()
        ));
    }

    Ok(root)
}

/// True when a walk must not enter `path`.
///
/// On macOS, a project that lives on another disk (`/Volumes/<name>/project`)
/// is still allowed. The volume root itself is not.
pub fn is_blocked(path: &Path) -> bool {
    if !is_under_protected(path) {
        return false;
    }

    #[cfg(target_os = "macos")]
    if let Ok(rest) = path.strip_prefix("/Volumes") {
        if rest.components().count() >= 2 {
            return false;
        }
    }

    true
}

fn is_filesystem_root(path: &Path) -> bool {
    let mut meaningful = path.components().filter(|component| {
        !matches!(
            component,
            Component::RootDir | Component::Prefix(_) | Component::CurDir
        )
    });
    meaningful.next().is_none()
}

fn is_under_protected(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let lower = path
            .to_string_lossy()
            .replace('/', "\\")
            .to_ascii_lowercase();
        return protected_prefixes().iter().any(|prefix| {
            let prefix = prefix.to_ascii_lowercase();
            lower == prefix || lower.starts_with(&(prefix + "\\"))
        });
    }

    #[cfg(not(windows))]
    {
        protected_prefixes()
            .iter()
            .any(|prefix| path == Path::new(prefix) || path.starts_with(prefix))
    }
}

fn protected_prefixes() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &[
            "/System",
            "/Library",
            "/usr",
            "/bin",
            "/sbin",
            "/private",
            "/Applications",
            "/Volumes",
        ]
    }

    #[cfg(target_os = "linux")]
    {
        &[
            "/proc", "/sys", "/dev", "/usr", "/bin", "/sbin", "/boot", "/run",
        ]
    }

    #[cfg(windows)]
    {
        &[
            r"C:\Windows",
            r"C:\Program Files",
            r"C:\Program Files (x86)",
        ]
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_filesystem_root() {
        let root = if cfg!(windows) {
            Path::new(r"C:\")
        } else {
            Path::new("/")
        };
        let err = validate_root(root).unwrap_err();
        assert!(err.contains("filesystem root"), "{err}");
    }

    #[test]
    fn rejects_system_directory() {
        let path = if cfg!(target_os = "macos") {
            Path::new("/usr")
        } else if cfg!(target_os = "linux") {
            Path::new("/usr")
        } else if cfg!(windows) {
            Path::new(r"C:\Windows")
        } else {
            return;
        };
        if !path.exists() {
            return;
        }
        let err = validate_root(path).unwrap_err();
        assert!(err.contains("system path"), "{err}");
    }

    #[test]
    fn allows_a_normal_project_directory() {
        // macOS temp lives under `/private`, which this program refuses to scan.
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/devclean-safety-test");
        fs::create_dir_all(&dir).unwrap();
        let canonical = validate_root(&dir).unwrap();
        assert!(canonical.is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn blocks_descendants_of_system_paths() {
        if cfg!(target_os = "macos") {
            assert!(is_blocked(Path::new("/usr/bin")));
            assert!(is_blocked(Path::new("/Volumes")));
            assert!(is_blocked(Path::new("/Volumes/Disk")));
            assert!(!is_blocked(Path::new("/Volumes/Disk/project")));
            assert!(!is_blocked(Path::new("/Users/me/code")));
        } else if cfg!(target_os = "linux") {
            assert!(is_blocked(Path::new("/proc/1")));
            assert!(!is_blocked(Path::new("/home/me/code")));
        }
    }

    #[test]
    fn filesystem_root_detection() {
        assert!(is_filesystem_root(Path::new("/")));
        assert!(!is_filesystem_root(Path::new("/Users")));
    }
}
