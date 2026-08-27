use humansize::{format_size, BINARY};
use jwalk::WalkDir;
use rayon::prelude::*;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Minimal y/n prompt over stdin — avoids pulling in a full terminal-UI
/// dependency (dialoguer/console) just for a single confirmation prompt.
pub fn confirm(prompt: &str, default: bool) -> io::Result<bool> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    print!("{prompt} {hint} ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let answer = line.trim().to_lowercase();
    Ok(match answer.as_str() {
        "" => default,
        "y" | "yes" => true,
        _ => false,
    })
}

pub fn fmt_size(bytes: u64) -> String {
    format_size(bytes, BINARY)
}

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Recursively walk a directory (in parallel) and return (path, size) for every regular file.
/// Symlinks are not followed. Errors reading individual entries are silently skipped.
pub fn scan_files(root: &Path) -> Vec<(PathBuf, u64)> {
    if !root.exists() {
        return Vec::new();
    }
    WalkDir::new(root)
        .skip_hidden(false)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let path = e.path();
            let meta = e.metadata().ok()?;
            Some((path, meta.len()))
        })
        .collect()
}

pub fn total_size(root: &Path) -> u64 {
    scan_files(root).iter().map(|(_, s)| *s).sum()
}

/// Delete every file under `root` whose top-level child name is not in `exclude`.
/// Directories are removed if left empty afterward. Returns (files_removed, bytes_freed).
pub fn purge_dir_excluding(root: &Path, exclude: &[&str]) -> (u64, u64) {
    if !root.exists() {
        return (0, 0);
    }
    let entries: Vec<PathBuf> = match fs::read_dir(root) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => return (0, 0),
    };

    let results: Vec<(u64, u64)> = entries
        .par_iter()
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            !exclude.contains(&name)
        })
        .map(|p| remove_path(p))
        .collect();

    let files = results.iter().map(|(f, _)| f).sum();
    let bytes = results.iter().map(|(_, b)| b).sum();
    (files, bytes)
}

fn remove_path(p: &Path) -> (u64, u64) {
    if p.is_dir() && !p.is_symlink() {
        let files = scan_files(p);
        let bytes: u64 = files.iter().map(|(_, s)| *s).sum();
        let count = files.len() as u64;
        match fs::remove_dir_all(p) {
            Ok(_) => (count, bytes),
            Err(_) => (0, 0),
        }
    } else {
        match fs::metadata(p) {
            Ok(m) => {
                let size = m.len();
                match fs::remove_file(p) {
                    Ok(_) => (1, size),
                    Err(_) => (0, 0),
                }
            }
            Err(_) => (0, 0),
        }
    }
}

/// Delete files under `root` older than `min_age_days`, matched by an optional extension filter.
pub fn purge_old_files(root: &Path, min_age_days: u64, ext_filter: Option<&[&str]>) -> (u64, u64) {
    if !root.exists() {
        return (0, 0);
    }
    let cutoff = min_age_days as u64 * 24 * 3600;
    let now = SystemTime::now();

    let files = scan_files(root);
    let results: Vec<(u64, u64)> = files
        .par_iter()
        .filter(|(path, _)| {
            if let Some(exts) = ext_filter {
                let ok = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| exts.contains(&e))
                    .unwrap_or(false);
                if !ok {
                    return false;
                }
            }
            true
        })
        .filter(|(path, _)| {
            fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|mtime| now.duration_since(mtime).ok())
                .map(|age| age.as_secs() >= cutoff)
                .unwrap_or(false)
        })
        .map(|(path, size)| match fs::remove_file(path) {
            Ok(_) => (1u64, *size),
            Err(_) => (0u64, 0u64),
        })
        .collect();

    let files_removed = results.iter().map(|(f, _)| f).sum();
    let bytes = results.iter().map(|(_, b)| b).sum();
    (files_removed, bytes)
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}
