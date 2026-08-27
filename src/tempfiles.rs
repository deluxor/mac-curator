use crate::util::{fmt_size, home_dir, is_root, purge_old_files, total_size};
use owo_colors::OwoColorize;
use std::path::PathBuf;

pub struct TempTarget {
    pub name: &'static str,
    pub path: PathBuf,
    pub min_age_days: u64,
    pub needs_root: bool,
}

pub fn targets() -> Vec<TempTarget> {
    let home = home_dir();
    vec![
        TempTarget {
            name: "/private/tmp",
            path: PathBuf::from("/private/tmp"),
            min_age_days: 3,
            needs_root: false,
        },
        TempTarget {
            name: "/private/var/tmp",
            path: PathBuf::from("/private/var/tmp"),
            min_age_days: 7,
            needs_root: true,
        },
        TempTarget {
            name: "User TemporaryItems",
            path: home.join("Library/Caches/TemporaryItems"),
            min_age_days: 0,
            needs_root: false,
        },
        TempTarget {
            name: "User logs (~/Library/Logs)",
            path: home.join("Library/Logs"),
            min_age_days: 14,
            needs_root: false,
        },
        TempTarget {
            name: "System logs (/Library/Logs)",
            path: PathBuf::from("/Library/Logs"),
            min_age_days: 14,
            needs_root: true,
        },
        TempTarget {
            name: "Diagnostic reports",
            path: home.join("Library/Logs/DiagnosticReports"),
            min_age_days: 7,
            needs_root: false,
        },
    ]
}

pub fn print_scan_report() {
    println!("{}", "Temp/log scan".bold());
    for t in targets() {
        let tag = if t.needs_root { " [root]" } else { "" };
        println!(
            "  {:<32} {:>10}{}",
            t.name,
            fmt_size(total_size(&t.path)),
            tag
        );
    }
}

pub fn clean_all(include_root_targets: bool) -> u64 {
    let mut freed = 0u64;
    for t in targets() {
        if t.needs_root && !(include_root_targets && is_root()) {
            continue;
        }
        let (files, bytes) = purge_old_files(&t.path, t.min_age_days, None);
        if files > 0 {
            println!(
                "  cleaned {:<32} {:>10} ({} files)",
                t.name,
                fmt_size(bytes),
                files
            );
        }
        freed += bytes;
    }
    freed
}
