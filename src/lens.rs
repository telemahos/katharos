//! Χάρτης χώρου: τι πιάνει χώρο σε κάθε φάκελο.

use crate::util::*;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
}

pub fn list(dir: &Path, c: &AtomicU64) -> Vec<Entry> {
    let mut v: Vec<Entry> = children(dir)
        .par_iter()
        .filter_map(|p| {
            let md = fs::symlink_metadata(p).ok()?;
            if md.file_type().is_symlink() {
                return None;
            }
            Some(Entry {
                path: p.clone(),
                name: file_name(p),
                size: path_size(p, c),
                is_dir: md.is_dir(),
            })
        })
        .filter(|e| e.size > 0)
        .collect();
    v.sort_by(|a, b| b.size.cmp(&a.size));
    v
}
