use std::ffi::OsStr;

/// Ecosystem a cache directory belongs to. Used only for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    JavaScript,
    Rust,
    Python,
    Other,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::JavaScript => "js",
            Kind::Rust => "rust",
            Kind::Python => "python",
            Kind::Other => "other",
        }
    }
}

struct Target {
    name: &'static str,
    kind: Kind,
}

/// Regenerable build and cache directories. Source trees, `.git`, and
/// virtualenvs are intentionally absent.
const TARGETS: &[Target] = &[
    Target {
        name: "node_modules",
        kind: Kind::JavaScript,
    },
    Target {
        name: ".next",
        kind: Kind::JavaScript,
    },
    Target {
        name: ".nuxt",
        kind: Kind::JavaScript,
    },
    Target {
        name: ".output",
        kind: Kind::JavaScript,
    },
    Target {
        name: ".turbo",
        kind: Kind::JavaScript,
    },
    Target {
        name: ".parcel-cache",
        kind: Kind::JavaScript,
    },
    Target {
        name: ".vite",
        kind: Kind::JavaScript,
    },
    Target {
        name: ".vercel",
        kind: Kind::JavaScript,
    },
    Target {
        name: "target",
        kind: Kind::Rust,
    },
    Target {
        name: "__pycache__",
        kind: Kind::Python,
    },
    Target {
        name: ".pytest_cache",
        kind: Kind::Python,
    },
    Target {
        name: ".mypy_cache",
        kind: Kind::Python,
    },
    Target {
        name: ".ruff_cache",
        kind: Kind::Python,
    },
    Target {
        name: ".tox",
        kind: Kind::Python,
    },
    Target {
        name: ".gradle",
        kind: Kind::Other,
    },
    Target {
        name: ".dart_tool",
        kind: Kind::Other,
    },
    Target {
        name: "coverage",
        kind: Kind::Other,
    },
    Target {
        name: ".nyc_output",
        kind: Kind::Other,
    },
];

/// Directory names that are never walked, even when they are not caches.
pub fn is_skipped_dir(name: &OsStr) -> bool {
    name == ".git"
}

/// Returns the cache kind when `name` is a known target directory.
///
/// Matching is case-insensitive on macOS and Windows, and exact on Linux.
pub fn lookup(name: &OsStr) -> Option<Kind> {
    TARGETS
        .iter()
        .find(|target| names_equal(name, OsStr::new(target.name)))
        .map(|target| target.kind)
}

fn names_equal(left: &OsStr, right: &OsStr) -> bool {
    if cfg!(any(windows, target_os = "macos")) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_known_caches() {
        assert_eq!(lookup(OsStr::new("node_modules")), Some(Kind::JavaScript));
        assert_eq!(lookup(OsStr::new("target")), Some(Kind::Rust));
        assert_eq!(lookup(OsStr::new("__pycache__")), Some(Kind::Python));
        assert_eq!(lookup(OsStr::new(".gradle")), Some(Kind::Other));
    }

    #[test]
    fn ignores_source_and_git() {
        assert!(lookup(OsStr::new("src")).is_none());
        assert!(lookup(OsStr::new(".git")).is_none());
        assert!(lookup(OsStr::new("dist")).is_none());
        assert!(lookup(OsStr::new(".venv")).is_none());
        assert!(is_skipped_dir(OsStr::new(".git")));
    }

    #[test]
    fn case_folding_matches_platform() {
        let hit = lookup(OsStr::new("Node_Modules"));
        if cfg!(any(windows, target_os = "macos")) {
            assert_eq!(hit, Some(Kind::JavaScript));
        } else {
            assert!(hit.is_none());
        }
    }
}
