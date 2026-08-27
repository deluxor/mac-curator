use crate::util::scan_files;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct DupGroup {
    pub size: u64,
    pub paths: Vec<PathBuf>,
}

impl DupGroup {
    pub fn wasted(&self) -> u64 {
        self.size * (self.paths.len() as u64 - 1)
    }
}

const PARTIAL_SAMPLE: usize = 64 * 1024;

fn partial_hash(path: &Path) -> Option<blake3::Hash> {
    let mut f = File::open(path).ok()?;
    let mut buf = vec![0u8; PARTIAL_SAMPLE];
    let n = f.read(&mut buf).ok()?;
    Some(blake3::hash(&buf[..n]))
}

fn full_hash(path: &Path) -> Option<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    let mut f = File::open(path).ok()?;
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hasher.finalize())
}

/// Finds duplicate files under `root` with minimal I/O:
///   1. bucket by exact file size (free — from metadata, no reads at all)
///   2. within each same-size bucket, bucket by a cheap hash of the first 64KB
///   3. only fully hash (BLAKE3, SIMD-accelerated) files that still collide
///
/// A file with a unique size in the tree is never opened. Size-buckets are
/// processed in parallel across a rayon thread pool since they're independent.
pub fn find_duplicates(root: &Path, min_size: u64) -> Vec<DupGroup> {
    let files = scan_files(root);

    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for (path, size) in files {
        if size < min_size {
            continue;
        }
        by_size.entry(size).or_default().push(path);
    }
    by_size.retain(|_, v| v.len() > 1);

    by_size
        .into_par_iter()
        .flat_map(|(size, paths)| {
            let mut by_partial: HashMap<blake3::Hash, Vec<PathBuf>> = HashMap::new();
            for p in &paths {
                if let Some(h) = partial_hash(p) {
                    by_partial.entry(h).or_default().push(p.clone());
                }
            }
            by_partial.retain(|_, v| v.len() > 1);

            by_partial
                .into_values()
                .flat_map(|candidates| {
                    let mut by_full: HashMap<blake3::Hash, Vec<PathBuf>> = HashMap::new();
                    for p in &candidates {
                        if let Some(h) = full_hash(p) {
                            by_full.entry(h).or_default().push(p.clone());
                        }
                    }
                    by_full
                        .into_values()
                        .filter(|v| v.len() > 1)
                        .map(|paths| DupGroup { size, paths })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
