use std::path::{Path, PathBuf};
use std::time::SystemTime;

use walkdir::WalkDir;

use crate::safety;
use crate::targets::{self, Kind};

/// A cache directory found under the scan root. Children are not walked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Find {
    pub path: PathBuf,
    pub name: String,
    pub kind: Kind,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scan {
    pub root: PathBuf,
    pub finds: Vec<Find>,
    pub warnings: Vec<String>,
}

/// Walk `root`, record known cache directories, and do not descend into them.
///
/// Symlinks are not followed. `.git` and OS-protected directories are skipped.
/// The scan root itself is never returned, even if its name is a target.
pub fn scan(root: &Path) -> Scan {
    let mut finds = Vec::new();
    let mut warnings = Vec::new();

    // `filter_entry` drops symlinks, files, `.git`, and OS paths. Cache
    // directories stay visible; `skip_current_dir` then keeps us out of them.
    let walker = WalkDir::new(root).follow_links(false).into_iter();
    let mut walker = walker.filter_entry(|entry| {
        if entry.depth() == 0 {
            return true;
        }
        if entry.file_type().is_symlink() || !entry.file_type().is_dir() {
            return false;
        }
        let name = entry.file_name();
        !targets::is_skipped_dir(name) && !safety::is_blocked(entry.path())
    });

    while let Some(item) = walker.next() {
        match item {
            Ok(entry) => {
                if entry.depth() == 0 || !entry.file_type().is_dir() {
                    continue;
                }
                let name = entry.file_name();
                let Some(kind) = targets::lookup(name) else {
                    continue;
                };
                if entry.path() == root {
                    continue;
                }
                let modified = entry.metadata().ok().and_then(|meta| meta.modified().ok());
                finds.push(Find {
                    path: entry.path().to_path_buf(),
                    name: name.to_string_lossy().into_owned(),
                    kind,
                    modified,
                });
                walker.skip_current_dir();
            }
            Err(err) => warnings.push(err.to_string()),
        }
    }

    finds.sort_by(|left, right| left.path.cmp(&right.path));
    warnings.sort();
    warnings.dedup();

    Scan {
        root: root.to_path_buf(),
        finds,
        warnings,
    }
}

/// UTC calendar date (`YYYY-MM-DD`) for a filesystem timestamp.
pub fn format_modified(modified: Option<SystemTime>) -> String {
    let Some(secs) = unix_secs(modified) else {
        return "-".to_string();
    };
    let (year, month, day) = civil_from_days((secs / 86_400) as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

/// UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`) for the restore log.
pub fn format_timestamp(time: SystemTime) -> String {
    let Some(secs) = unix_secs(Some(time)) else {
        return "-".to_string();
    };
    let (year, month, day) = civil_from_days((secs / 86_400) as i64);
    let tod = secs % 86_400;
    let hour = tod / 3_600;
    let minute = (tod % 3_600) / 60;
    let second = tod % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn unix_secs(time: Option<SystemTime>) -> Option<u64> {
    time?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

/// Howard Hinnant's `civil_from_days`, days since Unix epoch.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (if month <= 2 { y + 1 } else { y }) as i32;
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempTree {
        path: PathBuf,
    }

    impl TempTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "devclean-scan-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn records_caches_and_skips_their_contents() {
        let tree = TempTree::new();
        let modules = tree.path.join("app/node_modules");
        fs::create_dir_all(modules.join("pkg/node_modules")).unwrap();
        fs::write(modules.join("index.js"), b"console.log(1)").unwrap();
        fs::create_dir_all(tree.path.join("app/.next/cache")).unwrap();
        fs::create_dir_all(tree.path.join("app/src")).unwrap();
        fs::create_dir_all(tree.path.join("crates/foo/target/debug")).unwrap();
        fs::create_dir_all(tree.path.join("app/.git/objects")).unwrap();
        fs::create_dir_all(tree.path.join("app/.git/node_modules")).unwrap();
        fs::write(tree.path.join("README.md"), b"hi").unwrap();

        let scan = scan(&tree.path);
        let paths: Vec<_> = scan
            .finds
            .iter()
            .map(|find| find.path.strip_prefix(&tree.path).unwrap().to_path_buf())
            .collect();

        assert_eq!(
            paths,
            vec![
                PathBuf::from("app/.next"),
                PathBuf::from("app/node_modules"),
                PathBuf::from("crates/foo/target"),
            ]
        );
        assert!(scan.finds.iter().any(|find| find.kind == Kind::JavaScript));
        assert!(scan.finds.iter().any(|find| find.kind == Kind::Rust));
    }

    #[test]
    fn does_not_follow_symlinks() {
        let tree = TempTree::new();
        let real = tree.path.join("real-cache");
        fs::create_dir_all(real.join("nested")).unwrap();
        let link = tree.path.join("node_modules");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&real, &link).unwrap();

        let scan = scan(&tree.path);
        assert!(
            scan.finds.iter().all(|find| find.path != link),
            "symlink named node_modules must not be a hit: {:?}",
            scan.finds
        );
    }

    #[test]
    fn permission_error_is_a_warning_not_a_crash() {
        let tree = TempTree::new();
        fs::create_dir_all(tree.path.join("ok/node_modules")).unwrap();
        let scan = scan(&tree.path);
        assert_eq!(scan.finds.len(), 1);
        let _ = scan.warnings;
    }

    #[test]
    fn formats_unix_epoch() {
        let formatted = format_modified(Some(SystemTime::UNIX_EPOCH));
        assert_eq!(formatted, "1970-01-01");
    }
}
