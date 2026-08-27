use crate::util::{fmt_size, home_dir, is_root, purge_dir_excluding, total_size};
use owo_colors::OwoColorize;
use std::path::PathBuf;

pub struct CacheTarget {
    pub name: &'static str,
    pub path: PathBuf,
    pub needs_root: bool,
    /// Top-level child names inside `path` that must never be touched.
    pub exclude: &'static [&'static str],
}

/// Subdirectories of ~/Library/Caches that are risky to wipe blindly
/// (identity/keychain-adjacent or actively-synced state). Everything else
/// in user caches is safe to drop since apps regenerate it on demand.
const USER_CACHE_EXCLUDE: &[&str] = &["com.apple.iconservices.store", "CloudKit", "com.apple.akd"];

pub fn targets() -> Vec<CacheTarget> {
    let home = home_dir();
    vec![
        CacheTarget {
            name: "User app caches (~/Library/Caches)",
            path: home.join("Library/Caches"),
            needs_root: false,
            exclude: USER_CACHE_EXCLUDE,
        },
        CacheTarget {
            name: "User Safari/WebKit cache",
            path: home.join("Library/Caches/com.apple.Safari"),
            needs_root: false,
            exclude: &[],
        },
        CacheTarget {
            name: "QuickLook thumbnail cache",
            path: home.join("Library/Caches/com.apple.QuickLook.thumbnailcache"),
            needs_root: false,
            exclude: &[],
        },
        CacheTarget {
            name: "Xcode DerivedData",
            path: home.join("Library/Developer/Xcode/DerivedData"),
            needs_root: false,
            exclude: &[],
        },
        CacheTarget {
            name: "iOS/CoreSimulator caches",
            path: home.join("Library/Developer/CoreSimulator/Caches"),
            needs_root: false,
            exclude: &[],
        },
        CacheTarget {
            name: "System caches (/Library/Caches)",
            path: PathBuf::from("/Library/Caches"),
            needs_root: true,
            exclude: &[],
        },
        CacheTarget {
            name: "Mail downloaded attachments cache",
            path: home.join("Library/Containers/com.apple.mail/Data/Library/Mail Downloads"),
            needs_root: false,
            exclude: &[],
        },
        CacheTarget {
            name: "iOS/iPadOS software update files",
            path: home.join("Library/Application Support/MobileSync/Software Updates"),
            needs_root: false,
            exclude: &[],
        },
        CacheTarget {
            name: "Xcode iOS/watchOS/tvOS DeviceSupport",
            path: home.join("Library/Developer/Xcode/iOS DeviceSupport"),
            needs_root: false,
            exclude: &[],
        },
    ]
}

pub fn scan_report() -> Vec<(String, u64, bool)> {
    targets()
        .into_iter()
        .map(|t| (t.name.to_string(), total_size(&t.path), t.needs_root))
        .collect()
}

pub fn print_scan_report() {
    println!("{}", "Cache scan".bold());
    for (name, size, needs_root) in scan_report() {
        let tag = if needs_root { " [root]" } else { "" };
        println!("  {:<42} {:>10}{}", name, fmt_size(size), tag);
    }
}

/// Clean all cache targets the current privilege level allows. Returns bytes freed.
pub fn clean_all(include_root_targets: bool) -> u64 {
    let mut freed = 0u64;
    for t in targets() {
        if t.needs_root && !(include_root_targets && is_root()) {
            continue;
        }
        let (files, bytes) = purge_dir_excluding(&t.path, t.exclude);
        if files > 0 {
            println!(
                "  cleaned {:<40} {:>10} ({} files)",
                t.name,
                fmt_size(bytes),
                files
            );
        }
        freed += bytes;
    }
    freed
}
