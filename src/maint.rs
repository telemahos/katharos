//! Εργασίες συντήρησης macOS. Όσες θέλουν δικαιώματα διαχειριστή ζητούν
//! τον κωδικό μέσα από το κανονικό παράθυρο του macOS.

use crate::i18n::{t, tf, K};
use std::process::Command;

pub struct Task {
    pub name: K,
    pub desc: K,
    pub cmd: &'static str,
    pub admin: bool,
}

pub fn tasks() -> Vec<Task> {
    vec![
        Task {
            name: K::MRamName,
            desc: K::MRamDesc,
            cmd: "/usr/sbin/purge",
            admin: true,
        },
        Task {
            name: K::MDnsName,
            desc: K::MDnsDesc,
            cmd: "/usr/bin/dscacheutil -flushcache; /usr/bin/killall -HUP mDNSResponder",
            admin: true,
        },
        Task {
            name: K::MSpotName,
            desc: K::MSpotDesc,
            cmd: "/usr/bin/mdutil -E /",
            admin: true,
        },
        Task {
            name: K::MQLName,
            desc: K::MQLDesc,
            cmd: "/usr/bin/qlmanage -r && /usr/bin/qlmanage -r cache",
            admin: false,
        },
        Task {
            name: K::MLsName,
            desc: K::MLsDesc,
            cmd: "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -kill -r -domain local -domain system -domain user",
            admin: false,
        },
    ]
}

pub fn run(task: &Task) -> Result<String, String> {
    let out = if task.admin {
        let script = format!(
            "do shell script \"{}\" with administrator privileges",
            task.cmd.replace('\\', "\\\\").replace('"', "\\\"")
        );
        Command::new("osascript").args(["-e", &script]).output()
    } else {
        Command::new("/bin/sh").args(["-c", task.cmd]).output()
    };
    match out {
        Ok(o) if o.status.success() => Ok(tf(K::MDone, &[t(task.name)])),
        Ok(o) => {
            let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if e.contains("-128") {
                Err(tf(K::MCancel, &[t(task.name)]))
            } else {
                Err(format!("✖ {}: {}", t(task.name), e))
            }
        }
        Err(e) => Err(format!("✖ {}: {}", t(task.name), e)),
    }
}
