use crate::util::{fmt_size, home_dir, purge_dir_excluding, total_size};
use owo_colors::OwoColorize;
use std::path::PathBuf;

/// The user's own Trash plus the per-user Trash folder on every mounted
/// external/removable volume (macOS keeps a separate .Trashes/<uid> per volume).
pub fn trash_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![home_dir().join(".Trash")];

    let uid = unsafe { libc::getuid() };
    if let Ok(entries) = std::fs::read_dir("/Volumes") {
        for e in entries.flatten() {
            let vol_trash = e.path().join(".Trashes").join(uid.to_string());
            if vol_trash.exists() {
                dirs.push(vol_trash);
            }
        }
    }
    dirs
}

pub fn print_scan_report() {
    println!("{}", "Trash scan".bold());
    for dir in trash_dirs() {
        println!("  {:<32} {:>10}", dir.display(), fmt_size(total_size(&dir)));
    }
}

/// Permanently empties every discovered Trash. This is real, unrecoverable
/// deletion of items the user already chose to delete — still gated behind
/// the same confirm/--yes/--dry-run flow as the rest of `clean`.
pub fn empty_all() -> u64 {
    let mut freed = 0u64;
    for dir in trash_dirs() {
        let (files, bytes) = purge_dir_excluding(&dir, &[]);
        if files > 0 {
            println!("  emptied {:<32} {:>10} ({} files)", dir.display(), fmt_size(bytes), files);
        }
        freed += bytes;
    }
    freed
}
