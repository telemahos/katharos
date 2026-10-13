//! Μεγάλα & παλιά αρχεία και διπλότυπα.

use crate::util::*;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    /// Τελευταία χρήση = νεότερη από πρόσβαση/τροποποίηση.
    pub last_used: SystemTime,
}

impl FileInfo {
    fn from(p: &Path, md: &fs::Metadata, size: u64) -> Self {
        let m = md.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let a = md.accessed().unwrap_or(m);
        FileInfo { path: p.to_path_buf(), size, modified: m, last_used: a.max(m) }
    }
}

/// Βιβλιοθήκες/πακέτα που ΔΕΝ ανοίγουμε ούτε προτείνουμε για διαγραφή.
const SKIP_EXT: &[&str] = &[
    "app", "photoslibrary", "photolibrary", "musiclibrary", "tvlibrary", "imovielibrary",
    "fcpbundle", "lrdata", "aplibrary", "framework", "bundle", "plugin", "kext",
];
/// Πακέτα που εμφανίζονται ως ένα στοιχείο (π.χ. εικονικές μηχανές).
const LIST_EXT: &[&str] = &["pvm", "utm", "vmwarevm", "xcarchive", "sparsebundle"];

fn skip_name(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules"
}

pub fn scan_large(min: u64, c: &AtomicU64) -> Vec<FileInfo> {
    let h = home();
    let lib = h.join("Library");
    let out = Mutex::new(Vec::new());
    let skip = |p: &Path, name: &str| -> bool {
        if skip_name(name) || p == lib {
            return true;
        }
        let ext = ext_lower(p);
        if SKIP_EXT.contains(&ext.as_str()) {
            return true;
        }
        if LIST_EXT.contains(&ext.as_str()) {
            let s = path_size(p, c);
            if s >= min {
                if let Ok(md) = fs::symlink_metadata(p) {
                    out.lock().unwrap().push(FileInfo::from(p, &md, s));
                }
            }
            return true;
        }
        false
    };
    let visit = |p: &Path, md: &fs::Metadata| {
        let s = alloc_size(md);
        if s >= min {
            out.lock().unwrap().push(FileInfo::from(p, md, s));
        }
    };
    walk_files(&h, &skip, &visit, c);
    let mut v = out.into_inner().unwrap();
    v.sort_by(|a, b| b.size.cmp(&a.size));
    v
}

#[derive(Clone, Debug)]
pub struct DupGroup {
    pub size: u64,
    pub files: Vec<FileInfo>,
}

impl DupGroup {
    pub fn wasted(&self) -> u64 {
        self.size * (self.files.len() as u64 - 1)
    }
}

fn read_up_to(f: &mut File, buf: &mut [u8]) -> Option<usize> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(_) => return None,
        }
    }
    Some(n)
}

fn hash_partial(p: &Path) -> Option<[u8; 32]> {
    let mut f = File::open(p).ok()?;
    let mut buf = vec![0u8; 64 * 1024];
    let n = read_up_to(&mut f, &mut buf)?;
    Some(*blake3::hash(&buf[..n]).as_bytes())
}

fn hash_full(p: &Path) -> Option<[u8; 32]> {
    let mut f = File::open(p).ok()?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(*h.finalize().as_bytes())
}

/// Ομαδοποίηση: μέγεθος → hash πρώτων 64KB → πλήρες hash (BLAKE3).
pub fn scan_dupes(min: u64, c: &AtomicU64) -> Vec<DupGroup> {
    let h = home();
    let roots = ["Desktop", "Documents", "Downloads", "Pictures", "Movies", "Music"];
    let all = Mutex::new(Vec::<(FileInfo, u64, u64)>::new()); // (info, dev, inode)
    let skip = |p: &Path, name: &str| -> bool {
        skip_name(name) || SKIP_EXT.contains(&ext_lower(p).as_str())
    };
    let visit = |p: &Path, md: &fs::Metadata| {
        if md.len() >= min && alloc_size(md) > 0 {
            #[cfg(unix)]
            let (dev, ino) = {
                use std::os::unix::fs::MetadataExt;
                (md.dev(), md.ino())
            };
            #[cfg(not(unix))]
            let (dev, ino) = (0u64, 0u64);
            all.lock().unwrap().push((FileInfo::from(p, md, md.len()), dev, ino));
        }
    };
    roots.par_iter().for_each(|r| walk_files(&h.join(r), &skip, &visit, c));

    // 1. ανά μέγεθος (και αγνοούμε hard links του ίδιου αρχείου)
    let mut by_size: HashMap<u64, Vec<FileInfo>> = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    for (fi, dev, ino) in all.into_inner().unwrap() {
        if ino != 0 && !seen.insert((dev, ino)) {
            continue;
        }
        by_size.entry(fi.size).or_default().push(fi);
    }
    let cands: Vec<FileInfo> = by_size.into_values().filter(|v| v.len() > 1).flatten().collect();

    // 2. hash των πρώτων 64KB
    let partial: Vec<(FileInfo, [u8; 32])> = cands
        .into_par_iter()
        .filter_map(|f| hash_partial(&f.path).map(|h| (f, h)))
        .collect();
    let mut by_partial: HashMap<(u64, [u8; 32]), Vec<FileInfo>> = HashMap::new();
    for (f, h) in partial {
        by_partial.entry((f.size, h)).or_default().push(f);
    }
    let cands: Vec<FileInfo> = by_partial.into_values().filter(|v| v.len() > 1).flatten().collect();

    // 3. πλήρες hash
    let full: Vec<(FileInfo, [u8; 32])> = cands
        .into_par_iter()
        .filter_map(|f| hash_full(&f.path).map(|h| (f, h)))
        .collect();
    let mut by_full: HashMap<(u64, [u8; 32]), Vec<FileInfo>> = HashMap::new();
    for (f, h) in full {
        by_full.entry((f.size, h)).or_default().push(f);
    }
    let mut groups: Vec<DupGroup> = by_full
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|((size, _), mut files)| {
            files.sort_by_key(|f| f.modified); // το παλαιότερο πρώτο = το "πρωτότυπο"
            DupGroup { size, files }
        })
        .collect();
    groups.sort_by(|a, b| b.wasted().cmp(&a.wasted()));
    groups
}
