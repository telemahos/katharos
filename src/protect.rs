//! Προστασία: σάρωση ιών με ClamAV, καραντίνα, έλεγχος αυτόματης εκκίνησης,
//! υπογραφές εφαρμογών, επεκτάσεις browsers και κατάσταση ασφαλείας του macOS.
//!
//! Το ClamAV καλείται ως ξεχωριστό πρόγραμμα (clamscan / freshclam) — δεν
//! ενσωματώνεται, οπότε η άδεια MIT του Katharos δεν επηρεάζεται.

use crate::apps;
use crate::i18n::{t, tf, K};
use crate::util::*;
use rayon::prelude::*;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// ───────────────────────── Βοηθητικά ─────────────────────────

pub fn app_dir() -> PathBuf {
    home().join("Library/Application Support/Katharos")
}
pub fn db_dir() -> PathBuf {
    app_dir().join("clamav")
}
pub fn quarantine_dir() -> PathBuf {
    app_dir().join("Quarantine")
}

/// Βρίσκει ένα εκτελέσιμο. Οι εφαρμογές που ανοίγουν από το Finder δεν έχουν
/// το PATH του Τερματικού, γι' αυτό ελέγχουμε πρώτα τους φακέλους του Homebrew.
pub fn find_bin(name: &str) -> Option<PathBuf> {
    let mut c: Vec<PathBuf> = ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin", "/usr/bin"]
        .iter()
        .map(|d| Path::new(d).join(name))
        .collect();
    if let Some(p) = std::env::var_os("PATH") {
        c.extend(std::env::split_paths(&p).map(|d| d.join(name)));
    }
    c.into_iter().find(|p| p.is_file())
}

/// Τρέχει εντολή και επιστρέφει stdout+stderr (ή None αν δεν υπάρχει).
fn output(cmd: &str, args: &[&str]) -> Option<(String, bool)> {
    let o = Command::new(cmd).args(args).output().ok()?;
    let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&o.stderr));
    Some((s, o.status.success()))
}

pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn applescript_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Ανοίγει το Τερματικό και τρέχει μια εντολή, ώστε ο χρήστης να βλέπει την πρόοδο.
pub fn open_in_terminal(command: &str) {
    let script = format!(
        "tell application \"Terminal\"\nactivate\ndo script \"{}\"\nend tell",
        applescript_str(command)
    );
    let _ = Command::new("osascript").args(["-e", &script]).spawn();
}

pub fn open_url(url: &str) {
    let _ = Command::new("open").arg(url).spawn();
}

/// Παράθυρο επιλογής φακέλου του macOS.
pub fn choose_folder(prompt: &str) -> Option<PathBuf> {
    let o = Command::new("osascript")
        .args([
            "-e",
            "activate",
            "-e",
            &format!("POSIX path of (choose folder with prompt \"{}\")", applescript_str(prompt)),
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if o.status.success() && !s.is_empty() {
        Some(PathBuf::from(s))
    } else {
        None
    }
}

// ───────────────────────── ClamAV ─────────────────────────

const DB_EXT: &[&str] = &["cvd", "cld", "cud", "hdb", "hsb", "ndb", "ldb", "mdb", "msb", "yar", "yara"];

#[derive(Clone, Debug)]
pub struct ClamStatus {
    pub clamscan: Option<PathBuf>,
    pub has_brew: bool,
    pub db_files: usize,
    pub db_updated: Option<SystemTime>,
}

pub fn clam_status() -> ClamStatus {
    let mut files = 0;
    let mut newest: Option<SystemTime> = None;
    for p in children(&db_dir()) {
        if DB_EXT.contains(&ext_lower(&p).as_str()) {
            files += 1;
            if let Ok(m) = fs::metadata(&p).and_then(|m| m.modified()) {
                newest = Some(newest.map_or(m, |n| n.max(m)));
            }
        }
    }
    ClamStatus {
        clamscan: find_bin("clamscan"),
        has_brew: find_bin("brew").is_some(),
        db_files: files,
        db_updated: newest,
    }
}

/// Κατεβάζει/ενημερώνει τη βάση ιών σε δικό μας φάκελο (δεν χρειάζεται κωδικός).
pub fn update_db() -> Result<(), String> {
    let fc = find_bin("freshclam").ok_or_else(|| t(K::ClamNotFound).to_string())?;
    let db = db_dir();
    fs::create_dir_all(&db).map_err(|e| e.to_string())?;
    let conf = app_dir().join("freshclam.conf");
    fs::write(&conf, "DatabaseMirror database.clamav.net\n").map_err(|e| e.to_string())?;
    let o = Command::new(fc)
        .arg(format!("--config-file={}", conf.display()))
        .arg(format!("--datadir={}", db.display()))
        .arg("--stdout")
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        return Ok(());
    }
    let all = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    let last: Vec<&str> = all.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    Err(last[last.len().saturating_sub(2)..].join(" "))
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ScanKind {
    Quick,
    Full,
    Folder,
}

pub fn scan_targets(kind: ScanKind) -> Vec<PathBuf> {
    let h = home();
    let mut v = match kind {
        ScanKind::Quick => vec![h.join("Downloads"), h.join("Desktop"), h.join("Library/LaunchAgents")],
        ScanKind::Full => vec![h.clone(), PathBuf::from("/Applications")],
        ScanKind::Folder => vec![],
    };
    if kind != ScanKind::Folder {
        v.extend(
            ["/Library/LaunchAgents", "/Library/LaunchDaemons", "/Users/Shared"]
                .iter()
                .map(PathBuf::from),
        );
    }
    v
}

#[derive(Clone, Debug)]
pub struct Infection {
    pub path: PathBuf,
    pub threat: String,
    pub selected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ScanOutcome {
    pub scanned: u64,
    pub total: u64,
    pub infected: Vec<Infection>,
    pub error: Option<String>,
    pub cancelled: bool,
}

/// Χειρισμός σάρωσης από το UI (διακοπή).
#[derive(Default)]
pub struct ScanCtl {
    pub pid: AtomicU32,
    pub cancel: AtomicBool,
}

impl ScanCtl {
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        let pid = self.pid.load(Ordering::Relaxed);
        if pid != 0 {
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
        }
    }
}

/// "/path/file: Win.Test.EICAR_HDB-1 FOUND" → (path, threat)
pub fn parse_clam_line(line: &str) -> Option<(PathBuf, String)> {
    let body = line.trim_end().strip_suffix(" FOUND")?;
    let i = body.rfind(": ")?;
    Some((PathBuf::from(&body[..i]), body[i + 2..].to_string()))
}

/// Λίστα αρχείων προς σάρωση. Παραλείπει αρχεία iCloud που δεν έχουν κατέβει
/// (η ανάγνωσή τους θα τα κατέβαζε όλα) και τους δικούς μας φακέλους.
fn collect_files(targets: &[PathBuf], ctl: &ScanCtl) -> Vec<PathBuf> {
    let skip_q = quarantine_dir();
    let skip_db = db_dir();
    let out = Mutex::new(Vec::new());
    let dummy = AtomicU64::new(0);
    let skip = |p: &Path, name: &str| -> bool {
        ctl.cancel.load(Ordering::Relaxed)
            || name == "Mobile Documents"
            || name == "CloudStorage"
            || p == skip_q
            || p == skip_db
    };
    let visit = |p: &Path, md: &fs::Metadata| {
        if md.len() > 0 && alloc_size(md) == 0 {
            return;
        }
        if p.as_os_str().to_string_lossy().contains('\n') {
            return;
        }
        out.lock().unwrap().push(p.to_path_buf());
    };
    for target in targets {
        if target.is_dir() {
            walk_files(target, &skip, &visit, &dummy);
        } else if target.is_file() {
            out.lock().unwrap().push(target.clone());
        }
    }
    let mut v = out.into_inner().unwrap();
    v.sort();
    v.dedup();
    v
}

pub fn scan(targets: &[PathBuf], done: &AtomicU64, total: &AtomicU64, ctl: &ScanCtl) -> ScanOutcome {
    let mut res = ScanOutcome::default();
    let Some(clamscan) = find_bin("clamscan") else {
        res.error = Some(t(K::ClamNotFound).into());
        return res;
    };
    if clam_status().db_files == 0 {
        res.error = Some(t(K::ClamNoDb).into());
        return res;
    }
    let files = collect_files(targets, ctl);
    if ctl.cancel.load(Ordering::Relaxed) {
        res.cancelled = true;
        return res;
    }
    res.total = files.len() as u64;
    total.store(res.total, Ordering::Relaxed);
    if files.is_empty() {
        return res;
    }

    let list = app_dir().join("scan-list.txt");
    let written = (|| -> std::io::Result<()> {
        use std::os::unix::ffi::OsStrExt;
        fs::create_dir_all(app_dir())?;
        let mut f = std::io::BufWriter::new(fs::File::create(&list)?);
        for p in &files {
            f.write_all(p.as_os_str().as_bytes())?;
            f.write_all(b"\n")?;
        }
        f.flush()
    })();
    if let Err(e) = written {
        res.error = Some(e.to_string());
        return res;
    }

    let child = Command::new(clamscan)
        .arg(format!("--database={}", db_dir().display()))
        .arg(format!("--file-list={}", list.display()))
        .arg("--stdout")
        .arg("--no-summary")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            res.error = Some(e.to_string());
            return res;
        }
    };
    ctl.pid.store(child.id(), Ordering::Relaxed);
    if ctl.cancel.load(Ordering::Relaxed) {
        ctl.stop();
    }
    if let Some(out) = child.stdout.take() {
        let mut r = BufReader::new(out);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match r.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let line = String::from_utf8_lossy(&buf);
            if let Some((path, threat)) = parse_clam_line(&line) {
                res.infected.push(Infection { path, threat, selected: true });
                done.fetch_add(1, Ordering::Relaxed);
            } else if line.contains(": ") {
                done.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    let status = child.wait();
    ctl.pid.store(0, Ordering::Relaxed);
    let _ = fs::remove_file(&list);
    res.scanned = done.load(Ordering::Relaxed);
    res.cancelled = ctl.cancel.load(Ordering::Relaxed);
    if !res.cancelled {
        if let Ok(s) = status {
            // 0 = καθαρό, 1 = βρέθηκε ιός, 2 = σφάλμα
            if s.code() == Some(2) && res.scanned == 0 {
                res.error = Some("clamscan error".into());
            }
        }
    }
    res
}

// ───────────────────────── Καραντίνα ─────────────────────────

#[derive(Clone, Debug)]
pub struct QItem {
    pub dir: PathBuf,
    pub file: PathBuf,
    pub original: PathBuf,
    pub threat: String,
    pub size: u64,
    pub when: SystemTime,
    pub mode: u32,
    pub selected: bool,
}

const META: &str = ".katharos-meta";
const SYSTEM_PREFIXES: &[&str] = &["/System", "/usr", "/bin", "/sbin", "/private/var/db", "/Library/Apple"];

/// Μετακινεί ένα αρχείο σε καραντίνα: δεν μπορεί πλέον να εκτελεστεί, αλλά επαναφέρεται.
pub fn quarantine_one(path: &Path, threat: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let md = fs::symlink_metadata(path).map_err(|e| format!("{}: {}", tilde(path), e))?;
    if !md.is_file() || SYSTEM_PREFIXES.iter().any(|s| path.starts_with(s)) {
        return Err(tf(K::QRefused, &[&tilde(path)]));
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let dir = quarantine_dir().join(format!("{}-{:09}", now.as_secs(), now.subsec_nanos()));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(file_name(path));
    if fs::rename(path, &dest).is_err() {
        let r = fs::copy(path, &dest).and_then(|_| fs::remove_file(path));
        if let Err(e) = r {
            let _ = fs::remove_file(&dest);
            let _ = fs::remove_dir(&dir);
            return Err(format!("{}: {}", tilde(path), e));
        }
    }
    let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o400));
    let meta = format!(
        "path={}\nthreat={}\nmode={}\nsize={}\nwhen={}\n",
        path.display(),
        threat.replace('\n', " "),
        md.permissions().mode() & 0o7777,
        md.len(),
        now.as_secs()
    );
    fs::write(dir.join(META), meta).map_err(|e| e.to_string())
}

pub fn list_quarantine() -> Vec<QItem> {
    let mut v: Vec<QItem> = children(&quarantine_dir())
        .into_iter()
        .filter_map(|dir| {
            let meta = fs::read_to_string(dir.join(META)).ok()?;
            let get = |k: &str| {
                meta.lines()
                    .find_map(|l| l.split_once('=').filter(|(a, _)| *a == k).map(|(_, b)| b.to_string()))
                    .unwrap_or_default()
            };
            let file = children(&dir).into_iter().find(|p| file_name(p) != META)?;
            Some(QItem {
                original: PathBuf::from(get("path")),
                threat: get("threat"),
                mode: get("mode").parse().unwrap_or(0o644),
                size: get("size").parse().unwrap_or(0),
                when: UNIX_EPOCH + std::time::Duration::from_secs(get("when").parse().unwrap_or(0)),
                file,
                dir,
                selected: false,
            })
        })
        .collect();
    v.sort_by(|a, b| b.when.cmp(&a.when));
    v
}

pub fn restore(item: &QItem) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    if item.original.as_os_str().is_empty() || item.original.exists() {
        return Err(tf(K::RestoreExists, &[&tilde(&item.original)]));
    }
    if let Some(parent) = item.original.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if fs::rename(&item.file, &item.original).is_err() {
        fs::copy(&item.file, &item.original).map_err(|e| format!("{}: {}", tilde(&item.original), e))?;
    }
    let _ = fs::set_permissions(&item.original, fs::Permissions::from_mode(item.mode));
    delete_quarantined(item)
}

pub fn delete_quarantined(item: &QItem) -> Result<(), String> {
    if !item.dir.starts_with(quarantine_dir()) || item.dir == quarantine_dir() {
        return Err(tf(K::ProtectedSkip, &[&tilde(&item.dir)]));
    }
    fs::remove_dir_all(&item.dir).map_err(|e| format!("{}: {}", tilde(&item.dir), e))
}

// ───────────────────────── Υπογραφές κώδικα ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Sig {
    Apple,
    AppStore,
    DevId(String),
    Development(String),
    Adhoc,
    Unsigned,
    Missing,
    Unknown,
}

impl Sig {
    pub fn label(&self) -> String {
        match self {
            Sig::Apple => "Apple".into(),
            Sig::AppStore => "App Store".into(),
            Sig::DevId(n) => format!("Developer ID: {}", n),
            Sig::Development(n) => format!("Development: {}", n),
            Sig::Adhoc => t(K::FlagAdhoc).into(),
            Sig::Unsigned => t(K::FlagUnsigned).into(),
            Sig::Missing => t(K::FlagMissing).into(),
            Sig::Unknown => t(K::UnknownState).into(),
        }
    }
    pub fn flag(&self) -> Option<K> {
        match self {
            Sig::Adhoc => Some(K::FlagAdhoc),
            Sig::Unsigned => Some(K::FlagUnsigned),
            Sig::Missing => Some(K::FlagMissing),
            _ => None,
        }
    }
}

/// Ερμηνεύει την έξοδο του `codesign -dv --verbose=2`.
pub fn parse_codesign(s: &str) -> Sig {
    if s.contains("not signed at all") {
        return Sig::Unsigned;
    }
    if s.contains("Signature=adhoc") {
        return Sig::Adhoc;
    }
    let Some(a) = s.lines().find_map(|l| l.trim().strip_prefix("Authority=")) else {
        return Sig::Unknown;
    };
    if a.starts_with("Software Signing") {
        Sig::Apple
    } else if a.starts_with("Apple Mac OS Application Signing") {
        Sig::AppStore
    } else if let Some(n) = a.strip_prefix("Developer ID Application: ") {
        Sig::DevId(n.to_string())
    } else if let Some(n) = a.strip_prefix("Apple Development: ").or_else(|| a.strip_prefix("Mac Developer: ")) {
        Sig::Development(n.to_string())
    } else {
        Sig::DevId(a.to_string())
    }
}

pub fn signature(p: &Path) -> Sig {
    if !p.exists() {
        return Sig::Missing;
    }
    match Command::new("/usr/bin/codesign").args(["-dv", "--verbose=2"]).arg(p).output() {
        Ok(o) => parse_codesign(&String::from_utf8_lossy(&o.stderr)),
        Err(_) => Sig::Unknown,
    }
}

// ───────────────────────── Αυτόματη εκκίνηση ─────────────────────────

#[derive(Clone, Debug)]
pub struct StartupItem {
    pub plist: PathBuf,
    pub label: String,
    pub command: String,
    pub system: bool,
    pub sig: Sig,
    pub flags: Vec<K>,
}

const INTERPRETERS: &[&str] = &[
    "sh", "bash", "zsh", "dash", "ksh", "csh", "tcsh", "python", "python3", "perl", "ruby", "node", "osascript", "curl", "php",
];
const TEMP_PLACES: &[&str] = &["/tmp/", "/private/tmp/", "/var/tmp/", "/private/var/tmp/", "/Users/Shared/"];

pub fn startup_dirs() -> Vec<(PathBuf, bool)> {
    vec![
        (home().join("Library/LaunchAgents"), false),
        (PathBuf::from("/Library/LaunchAgents"), true),
        (PathBuf::from("/Library/LaunchDaemons"), true),
    ]
}

pub fn startup_items() -> Vec<StartupItem> {
    let plists: Vec<(PathBuf, bool)> = startup_dirs()
        .into_iter()
        .flat_map(|(d, sys)| children(&d).into_iter().filter(|p| ext_lower(p) == "plist").map(move |p| (p, sys)))
        .collect();
    let mut v: Vec<StartupItem> = plists.par_iter().map(|(p, sys)| inspect_plist(p, *sys)).collect();
    v.sort_by(|a, b| {
        b.flags
            .len()
            .cmp(&a.flags.len())
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    v
}

pub fn inspect_plist(p: &Path, system: bool) -> StartupItem {
    let mut label = file_name(p).trim_end_matches(".plist").to_string();
    let mut args: Vec<String> = Vec::new();
    if let Ok(v) = plist::Value::from_file(p) {
        if let Some(d) = v.as_dictionary() {
            if let Some(l) = d.get("Label").and_then(|x| x.as_string()) {
                label = l.to_string();
            }
            if let Some(a) = d.get("ProgramArguments").and_then(|x| x.as_array()) {
                args = a.iter().filter_map(|x| x.as_string().map(String::from)).collect();
            }
            if let Some(pr) = d.get("Program").and_then(|x| x.as_string()) {
                if args.is_empty() {
                    args.push(pr.into());
                } else {
                    args[0] = pr.into();
                }
            }
        }
    }
    let program = args.first().map(PathBuf::from);
    let sig = program.as_deref().map(signature).unwrap_or(Sig::Missing);
    let mut flags: Vec<K> = sig.flag().into_iter().collect();
    if let Some(pr) = &program {
        let s = pr.to_string_lossy();
        if pr.components().any(|c| {
            let n = c.as_os_str().to_string_lossy();
            n.len() > 1 && n.starts_with('.')
        }) {
            flags.push(K::FlagHidden);
        }
        if TEMP_PLACES.iter().any(|x| s.starts_with(x)) {
            flags.push(K::FlagTemp);
        }
        let base = file_name(pr);
        let short = base.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        if INTERPRETERS.contains(&base.as_str()) || INTERPRETERS.contains(&short) {
            flags.push(K::FlagScript);
        }
    }
    StartupItem { plist: p.to_path_buf(), label, command: args.join(" "), system, sig, flags }
}

/// Σταματά το στοιχείο και μετακινεί το .plist στον Κάδο.
/// Για στοιχεία συστήματος ζητείται ο κωδικός μέσω του παραθύρου του macOS.
pub fn remove_startup(item: &StartupItem) -> Result<(), String> {
    let uid = unsafe { libc::getuid() };
    let parent = item.plist.parent().map(Path::to_path_buf).unwrap_or_default();
    if !startup_dirs().iter().any(|(d, _)| *d == parent) {
        return Err(tf(K::ProtectedSkip, &[&tilde(&item.plist)]));
    }
    if !item.system {
        let _ = Command::new("launchctl")
            .arg("bootout")
            .arg(format!("gui/{}", uid))
            .arg(&item.plist)
            .output();
        let (ok, errs) = trash_many(std::slice::from_ref(&item.plist));
        return if ok.is_empty() { Err(errs.join("; ")) } else { Ok(()) };
    }
    let p = sh_quote(&item.plist.to_string_lossy());
    let trash = sh_quote(&home().join(".Trash").to_string_lossy());
    let cmd = format!(
        "/bin/launchctl bootout system {p} 2>/dev/null; /bin/launchctl bootout gui/{uid} {p} 2>/dev/null; /bin/mv -f {p} {trash}/"
    );
    let script = format!("do shell script \"{}\" with administrator privileges", applescript_str(&cmd));
    let o = Command::new("osascript").args(["-e", &script]).output().map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}

// ───────────────────────── Εφαρμογές ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Gk {
    Notarized,
    AppStore,
    Accepted,
    Rejected,
    Unknown,
}

impl Gk {
    pub fn label(&self) -> String {
        match self {
            Gk::Notarized => t(K::GkNotarized).into(),
            Gk::AppStore => "App Store".into(),
            Gk::Accepted => t(K::GkAccepted).into(),
            Gk::Rejected => t(K::GkRejected).into(),
            Gk::Unknown => t(K::UnknownState).into(),
        }
    }
}

/// Ερμηνεύει την έξοδο του `spctl --assess --type execute -vv`.
pub fn parse_spctl(s: &str) -> Gk {
    let src = s.lines().find_map(|l| l.trim().strip_prefix("source=")).unwrap_or("");
    if s.contains(": rejected") {
        Gk::Rejected
    } else if s.contains(": accepted") {
        if src.contains("App Store") {
            Gk::AppStore
        } else if src.contains("Notarized") {
            Gk::Notarized
        } else {
            Gk::Accepted
        }
    } else {
        Gk::Unknown
    }
}

#[derive(Clone, Debug)]
pub struct AppCheck {
    pub name: String,
    pub path: PathBuf,
    pub sig: Sig,
    pub gk: Gk,
    pub flags: Vec<K>,
}

pub fn check_apps(c: &AtomicU64) -> Vec<AppCheck> {
    let mut v: Vec<AppCheck> = apps::list(c)
        .par_iter()
        .map(|a| {
            let sig = signature(&a.path);
            let gk = Command::new("/usr/sbin/spctl")
                .args(["--assess", "--type", "execute", "-vv"])
                .arg(&a.path)
                .output()
                .map(|o| parse_spctl(&String::from_utf8_lossy(&o.stderr)))
                .unwrap_or(Gk::Unknown);
            let mut flags: Vec<K> = sig.flag().into_iter().collect();
            if gk == Gk::Rejected {
                flags.push(K::GkRejected);
            }
            AppCheck { name: a.name.clone(), path: a.path.clone(), sig, gk, flags }
        })
        .collect();
    v.sort_by(|a, b| {
        b.flags
            .len()
            .cmp(&a.flags.len())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    v
}

// ───────────────────────── Επεκτάσεις browsers ─────────────────────────

#[derive(Clone, Debug)]
pub struct Ext {
    pub browser: String,
    pub name: String,
    pub version: String,
    pub id: String,
    pub path: PathBuf,
    pub all_sites: bool,
    pub sensitive: Vec<String>,
    pub enabled: Option<bool>,
}

const CHROMIUM: &[(&str, &str)] = &[
    ("Google/Chrome", "Chrome"),
    ("BraveSoftware/Brave-Browser", "Brave"),
    ("Microsoft Edge", "Edge"),
    ("Vivaldi", "Vivaldi"),
    ("Chromium", "Chromium"),
    ("Arc/User Data", "Arc"),
    ("com.operasoftware.Opera", "Opera"),
];
const SENSITIVE: &[&str] = &[
    "cookies", "history", "webRequest", "webRequestBlocking", "clipboardRead", "nativeMessaging", "proxy", "debugger",
    "management", "privacy", "downloads", "tabs", "scripting", "webNavigation",
];
const ALL_SITES: &[&str] = &["<all_urls>", "*://*/*", "http://*/*", "https://*/*", "*://*/", "http://*/", "https://*/"];

fn json(p: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(p).ok()?).ok()
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

fn make_ext(browser: String, name: String, version: String, id: String, path: PathBuf, perms: &[String], enabled: Option<bool>) -> Ext {
    let all_sites = perms.iter().any(|p| ALL_SITES.contains(&p.as_str()));
    let mut sensitive: Vec<String> = SENSITIVE
        .iter()
        .filter(|s| perms.iter().any(|p| p == *s))
        .map(|s| s.to_string())
        .collect();
    sensitive.dedup();
    Ext { browser, name, version, id, path, all_sites, sensitive, enabled }
}

/// Όνομα επέκτασης: αν είναι "__MSG_x__", διαβάζεται από τα _locales.
fn localized(dir: &Path, m: &Value, raw: &str) -> String {
    if let Some(key) = raw.strip_prefix("__MSG_").and_then(|k| k.strip_suffix("__")) {
        let mut locs: Vec<String> = m["default_locale"].as_str().map(String::from).into_iter().collect();
        locs.extend(["en".to_string(), "en_US".to_string()]);
        for l in locs {
            if let Some(msgs) = json(&dir.join("_locales").join(&l).join("messages.json")) {
                if let Some(obj) = msgs.as_object() {
                    for (k, v) in obj {
                        if k.eq_ignore_ascii_case(key) {
                            if let Some(s) = v["message"].as_str() {
                                return s.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    raw.to_string()
}

fn manifest_perms(m: &Value) -> Vec<String> {
    let mut perms = strs(&m["permissions"]);
    perms.extend(strs(&m["host_permissions"]));
    if let Some(cs) = m["content_scripts"].as_array() {
        for c in cs {
            perms.extend(strs(&c["matches"]));
        }
    }
    perms
}

pub fn chromium_exts(base: &Path, browser: &str) -> Vec<Ext> {
    let mut out = Vec::new();
    for profile in children(base) {
        let ext_dir = profile.join("Extensions");
        if !ext_dir.is_dir() {
            continue;
        }
        let pname = file_name(&profile);
        let shown = if pname == "Default" { browser.to_string() } else { format!("{} · {}", browser, pname) };
        for id_dir in children(&ext_dir) {
            let id = file_name(&id_dir);
            if id == "Temp" || id.starts_with('.') {
                continue;
            }
            let mut versions: Vec<PathBuf> =
                children(&id_dir).into_iter().filter(|p| p.join("manifest.json").is_file()).collect();
            versions.sort();
            let Some(vdir) = versions.pop() else { continue };
            let Some(m) = json(&vdir.join("manifest.json")) else { continue };
            let name = localized(&vdir, &m, m["name"].as_str().unwrap_or(&id));
            let version = m["version"].as_str().unwrap_or("").to_string();
            out.push(make_ext(shown.clone(), name, version, id, vdir, &manifest_perms(&m), None));
        }
    }
    out
}

fn firefox_exts() -> Vec<Ext> {
    let mut out = Vec::new();
    for prof in children(&home().join("Library/Application Support/Firefox/Profiles")) {
        let Some(j) = json(&prof.join("extensions.json")) else { continue };
        let Some(addons) = j["addons"].as_array() else { continue };
        let pname = file_name(&prof);
        let pname = pname.split_once('.').map(|(_, b)| b.to_string()).unwrap_or(pname);
        for a in addons {
            if a["type"].as_str() != Some("extension") || a["location"].as_str() != Some("app-profile") {
                continue;
            }
            let id = a["id"].as_str().unwrap_or("").to_string();
            let name = a["defaultLocale"]["name"].as_str().unwrap_or(&id).to_string();
            let mut perms = strs(&a["userPermissions"]["permissions"]);
            perms.extend(strs(&a["userPermissions"]["origins"]));
            out.push(make_ext(
                format!("Firefox · {}", pname),
                name,
                a["version"].as_str().unwrap_or("").to_string(),
                id,
                a["path"].as_str().map(PathBuf::from).unwrap_or_else(|| prof.join("extensions")),
                &perms,
                a["active"].as_bool(),
            ));
        }
    }
    out
}

/// Ερμηνεύει την έξοδο του `pluginkit -mAvvv -p <sdk>`.
/// Επιστρέφει (id, εμφανιζόμενο όνομα, διαδρομή .appex, ενεργή).
pub fn parse_pluginkit(s: &str) -> Vec<(String, String, PathBuf, bool)> {
    let mut out = Vec::new();
    let mut cur: Option<(String, String, PathBuf, bool)> = None;
    for line in s.lines() {
        let tr = line.trim();
        if let Some((k, v)) = tr.split_once(" = ") {
            if let Some(c) = cur.as_mut() {
                match k.trim() {
                    "Path" => c.2 = PathBuf::from(v.trim()),
                    "Display Name" => c.1 = v.trim().to_string(),
                    _ => {}
                }
            }
            continue;
        }
        let enabled = !tr.starts_with('-');
        let id_part = tr.trim_start_matches(|ch: char| "+-!=? \t".contains(ch));
        if let Some(i) = id_part.find('(') {
            if i > 0 && id_part.ends_with(')') {
                if let Some(c) = cur.take() {
                    out.push(c);
                }
                cur = Some((id_part[..i].trim().to_string(), String::new(), PathBuf::new(), enabled));
            }
        }
    }
    out.extend(cur);
    out
}

fn safari_exts() -> Vec<Ext> {
    let mut out = Vec::new();
    for sdk in ["com.apple.Safari.web-extension", "com.apple.Safari.extension", "com.apple.Safari.content-blocker"] {
        let Some((s, _)) = output("/usr/bin/pluginkit", &["-mAvvv", "-p", sdk]) else { continue };
        for (id, name, path, enabled) in parse_pluginkit(&s) {
            let m = json(&path.join("Contents/Resources/manifest.json")).unwrap_or(Value::Null);
            let name = if name.is_empty() { id.clone() } else { name };
            let version = m["version"].as_str().unwrap_or("").to_string();
            out.push(make_ext("Safari".into(), name, version, id, path, &manifest_perms(&m), Some(enabled)));
        }
    }
    out
}

pub fn browser_extensions() -> Vec<Ext> {
    let support = home().join("Library/Application Support");
    let mut v: Vec<Ext> = CHROMIUM
        .par_iter()
        .flat_map(|(dir, name)| chromium_exts(&support.join(dir), name))
        .collect();
    v.extend(firefox_exts());
    v.extend(safari_exts());
    v.sort_by(|a, b| {
        b.all_sites
            .cmp(&a.all_sites)
            .then_with(|| b.sensitive.len().cmp(&a.sensitive.len()))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    v
}

// ───────────────────────── Κατάσταση ασφαλείας ─────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Level {
    Good,
    Warn,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct Check {
    pub name: K,
    pub level: Level,
    pub detail: String,
    pub warn: K,
    pub url: &'static str,
}

const SEC_PANE: &str = "x-apple.systempreferences:com.apple.preference.security";

fn on_off(level: Level, good_means_on: bool) -> String {
    match (level, good_means_on) {
        (Level::Good, true) | (Level::Warn, false) => t(K::On).into(),
        (Level::Warn, true) | (Level::Good, false) => t(K::Off).into(),
        (Level::Unknown, _) => t(K::UnknownState).into(),
    }
}

fn level_from(out: Option<(String, bool)>, good: &str, bad: &str) -> Level {
    match out {
        Some((s, _)) => {
            let s = s.to_lowercase();
            if s.contains(bad) {
                Level::Warn
            } else if s.contains(good) {
                Level::Good
            } else {
                Level::Unknown
            }
        }
        None => Level::Unknown,
    }
}

pub fn security_checks() -> Vec<Check> {
    let mut v = Vec::new();

    let sip = output("/usr/bin/csrutil", &["status"]).map(|(s, ok)| (s.lines().next().unwrap_or("").to_string(), ok));
    let l = level_from(sip, "enabled", "disabled");
    v.push(Check { name: K::CkSip, level: l, detail: on_off(l, true), warn: K::CkSipWarn, url: SEC_PANE });

    let l = level_from(output("/usr/sbin/spctl", &["--status"]), "assessments enabled", "assessments disabled");
    v.push(Check { name: K::CkGk, level: l, detail: on_off(l, true), warn: K::CkGkWarn, url: SEC_PANE });

    let l = level_from(output("/usr/bin/fdesetup", &["status"]), "filevault is on", "filevault is off");
    v.push(Check {
        name: K::CkFv,
        level: l,
        detail: on_off(l, true),
        warn: K::CkFvWarn,
        url: "x-apple.systempreferences:com.apple.preference.security?FileVault",
    });

    let l = level_from(
        output("/usr/libexec/ApplicationFirewall/socketfilterfw", &["--getglobalstate"]),
        "enabled",
        "disabled",
    );
    v.push(Check {
        name: K::CkFw,
        level: l,
        detail: on_off(l, true),
        warn: K::CkFwWarn,
        url: "x-apple.systempreferences:com.apple.preference.security?Firewall",
    });

    let xp = Path::new("/Library/Apple/System/Library/CoreServices/XProtect.bundle/Contents/Info.plist");
    let version = plist::Value::from_file(xp).ok().and_then(|v| {
        v.as_dictionary()
            .and_then(|d| d.get("CFBundleShortVersionString"))
            .and_then(|x| x.as_string())
            .map(String::from)
    });
    let modified = fs::metadata(xp).and_then(|m| m.modified()).ok();
    let (l, detail) = match (version, modified) {
        (Some(ver), Some(m)) => (
            if age_days(m) > 90 { Level::Warn } else { Level::Good },
            tf(K::CkXpDetail, &[&ver, &age_text(m)]),
        ),
        _ => (Level::Unknown, t(K::UnknownState).into()),
    };
    v.push(Check {
        name: K::CkXp,
        level: l,
        detail,
        warn: K::CkXpWarn,
        url: "x-apple.systempreferences:com.apple.preferences.softwareupdate",
    });

    let crit = output("/usr/bin/defaults", &["read", "/Library/Preferences/com.apple.SoftwareUpdate", "CriticalUpdateInstall"]);
    let conf = output("/usr/bin/defaults", &["read", "/Library/Preferences/com.apple.SoftwareUpdate", "ConfigDataInstall"]);
    let vals: Vec<String> = [crit, conf]
        .into_iter()
        .flatten()
        .filter(|(_, ok)| *ok)
        .map(|(s, _)| s.trim().to_string())
        .collect();
    let l = if vals.iter().any(|x| x == "0") {
        Level::Warn
    } else if vals.iter().any(|x| x == "1") {
        Level::Good
    } else {
        Level::Unknown
    };
    v.push(Check {
        name: K::CkUpdates,
        level: l,
        detail: on_off(l, true),
        warn: K::CkUpdatesWarn,
        url: "x-apple.systempreferences:com.apple.preferences.softwareupdate",
    });

    let auto = output("/usr/bin/defaults", &["read", "/Library/Preferences/com.apple.loginwindow", "autoLoginUser"]);
    let l = match auto {
        Some((s, true)) if !s.trim().is_empty() => Level::Warn,
        Some(_) => Level::Good,
        None => Level::Unknown,
    };
    v.push(Check {
        name: K::CkAutoLogin,
        level: l,
        detail: on_off(l, false),
        warn: K::CkAutoLoginWarn,
        url: "x-apple.systempreferences:com.apple.preferences.users",
    });
    v
}
