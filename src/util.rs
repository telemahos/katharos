//! Κοινές βοηθητικές συναρτήσεις: μέγεθος φακέλων, ασφαλής διαγραφή, μορφοποίηση.

use crate::i18n::{t, tf, K};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// Πραγματικός χώρος στο δίσκο (blocks), όχι λογικό μέγεθος.
/// Έτσι αρχεία iCloud που δεν έχουν κατέβει (0 blocks) δεν "φουσκώνουν" τα νούμερα.
pub fn alloc_size(md: &fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        md.blocks() * 512
    }
    #[cfg(not(unix))]
    {
        md.len()
    }
}

pub fn children(p: &Path) -> Vec<PathBuf> {
    match fs::read_dir(p) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string_lossy().to_string())
}

pub fn ext_lower(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn skip_virtual(p: &Path) -> bool {
    let s = p.as_os_str();
    s.len() < 24 && (p == Path::new("/System/Volumes") || p == Path::new("/Volumes") || p == Path::new("/dev"))
}

/// Παράλληλος υπολογισμός μεγέθους (δεν ακολουθεί symlinks).
pub fn path_size(p: &Path, c: &AtomicU64) -> u64 {
    if skip_virtual(p) {
        return 0;
    }
    let md = match fs::symlink_metadata(p) {
        Ok(m) => m,
        Err(_) => return 0,
    };
    c.fetch_add(1, Ordering::Relaxed);
    if md.is_dir() {
        alloc_size(&md) + children(p).par_iter().map(|x| path_size(x, c)).sum::<u64>()
    } else {
        alloc_size(&md)
    }
}

/// Παράλληλη διάσχιση αρχείων. `skip_dir` επιστρέφει true για φακέλους που δεν θέλουμε να ανοίξουμε.
pub fn walk_files<S, V>(root: &Path, skip_dir: &S, visit: &V, c: &AtomicU64)
where
    S: Fn(&Path, &str) -> bool + Sync,
    V: Fn(&Path, &fs::Metadata) + Sync,
{
    children(root).par_iter().for_each(|p| {
        let md = match fs::symlink_metadata(p) {
            Ok(m) => m,
            Err(_) => return,
        };
        c.fetch_add(1, Ordering::Relaxed);
        if md.is_dir() {
            let name = file_name(p);
            if !skip_dir(p, &name) {
                walk_files(p, skip_dir, visit, c);
            }
        } else if md.is_file() {
            visit(p, &md);
        }
    });
}

pub fn human(b: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1000.0 && i < units.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{} B", b)
    } else if v >= 100.0 {
        format!("{:.0} {}", v, units[i])
    } else {
        format!("{:.1} {}", v, units[i])
    }
}

/// Εμφάνιση διαδρομής με ~ αντί για τον αρχικό φάκελο.
pub fn tilde(p: &Path) -> String {
    let h = home();
    match p.strip_prefix(&h) {
        Ok(rel) if rel.as_os_str().is_empty() => "~".into(),
        Ok(rel) => format!("~/{}", rel.display()),
        Err(_) => p.display().to_string(),
    }
}

pub fn age_days(t: SystemTime) -> u64 {
    SystemTime::now()
        .duration_since(t)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0)
}

pub fn age_text(when: SystemTime) -> String {
    let d = age_days(when);
    match d {
        0 => t(K::AgeToday).into(),
        1 => t(K::AgeYesterday).into(),
        2..=59 => tf(K::AgeDays, &[&d.to_string()]),
        60..=729 => tf(K::AgeMonths, &[&(d / 30).to_string()]),
        _ => tf(K::AgeYears, &[&(d / 365).to_string()]),
    }
}

/// (συνολικός, διαθέσιμος) χώρος του δίσκου δεδομένων.
pub fn disk_space() -> (u64, u64) {
    let target = if Path::new("/System/Volumes/Data").exists() {
        "/System/Volumes/Data"
    } else {
        "/"
    };
    let cpath = std::ffi::CString::new(target).unwrap();
    unsafe {
        #[cfg(target_os = "macos")]
        {
            let mut s: libc::statfs = std::mem::zeroed();
            if libc::statfs(cpath.as_ptr(), &mut s) == 0 {
                let bs = s.f_bsize as u64;
                return (s.f_blocks as u64 * bs, s.f_bavail as u64 * bs);
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut s: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(cpath.as_ptr(), &mut s) == 0 {
                let f = s.f_frsize as u64;
                return (s.f_blocks as u64 * f, s.f_bavail as u64 * f);
            }
        }
    }
    (0, 0)
}

const HOME_PROTECTED: &[&str] = &[
    "Library", "Documents", "Desktop", "Downloads", "Pictures", "Movies", "Music",
    "Applications", "Public", ".Trash",
];
const LIB_PROTECTED: &[&str] = &[
    "Library/Caches", "Library/Logs", "Library/Application Support", "Library/Preferences",
    "Library/Containers", "Library/Group Containers", "Library/Saved Application State",
    "Library/Mail", "Library/Developer", "Library/Mobile Documents", "Library/CloudStorage",
    "Library/Keychains",
];

/// Δίχτυ ασφαλείας: η εφαρμογή δεν αγγίζει τίποτα έξω από τον φάκελο χρήστη
/// (εκτός από .app στο /Applications) ούτε τους βασικούς φακέλους του συστήματος.
pub fn is_safe_target(p: &Path) -> bool {
    if p.starts_with("/Applications") {
        return ext_lower(p) == "app" && p.components().count() >= 3;
    }
    let h = home();
    if h == Path::new("/") || !p.starts_with(&h) || p == h {
        return false;
    }
    if p.components().any(|c| c.as_os_str() == "..") {
        return false;
    }
    let depth = p.components().count() - h.components().count();
    if depth == 1 && HOME_PROTECTED.contains(&file_name(p).as_str()) {
        return false;
    }
    !LIB_PROTECTED.iter().any(|lp| p == h.join(lp))
}

/// Οριστική διαγραφή (για cache/logs που ξαναδημιουργούνται).
pub fn remove_permanent(p: &Path) -> Result<(), String> {
    if !is_safe_target(p) {
        return Err(tf(K::ProtectedSkip, &[&tilde(p)]));
    }
    let md = fs::symlink_metadata(p).map_err(|e| format!("{}: {}", tilde(p), e))?;
    let r = if md.is_dir() { fs::remove_dir_all(p) } else { fs::remove_file(p) };
    r.map_err(|e| format!("{}: {}", tilde(p), e))
}

/// Αδειάζει τα περιεχόμενα ενός φακέλου αλλά κρατά τον ίδιο τον φάκελο.
pub fn empty_dir(p: &Path) -> Vec<String> {
    children(p)
        .par_iter()
        .filter_map(|ch| remove_permanent(ch).err())
        .collect()
}

/// Μετακίνηση στον Κάδο (ανακτήσιμο). Επιστρέφει (επιτυχημένα, σφάλματα).
pub fn trash_many(paths: &[PathBuf]) -> (Vec<PathBuf>, Vec<String>) {
    let mut errs = Vec::new();
    let mut safe = Vec::new();
    for p in paths {
        if is_safe_target(p) {
            safe.push(p.clone());
        } else {
            errs.push(tf(K::ProtectedSkip, &[&tilde(p)]));
        }
    }
    if safe.is_empty() {
        return (safe, errs);
    }
    if trash::delete_all(&safe).is_ok() {
        return (safe, errs);
    }
    let mut ok = Vec::new();
    for p in safe {
        if !p.exists() {
            ok.push(p);
            continue;
        }
        match trash::delete(&p) {
            Ok(()) => ok.push(p),
            Err(e) => errs.push(format!("{}: {}", tilde(&p), e)),
        }
    }
    (ok, errs)
}

pub fn reveal_in_finder(p: &Path) {
    let _ = Command::new("open").arg("-R").arg(p).spawn();
}

pub fn open_path(p: &Path) {
    let _ = Command::new("open").arg(p).spawn();
}
