//! Οθόνη «Προστασία»: καρτέλες για ιούς, αυτόματη εκκίνηση, υπογραφές εφαρμογών,
//! επεκτάσεις browsers, κατάσταση ασφαλείας και καραντίνα.

use super::*;
use crate::protect::{self, AppCheck, Check, Ext, Level, QItem, ScanCtl, ScanKind, ScanOutcome, StartupItem};
use std::sync::atomic::AtomicU64;

#[derive(Clone, Copy, PartialEq)]
pub enum PTab {
    Scan,
    Startup,
    Apps,
    Ext,
    Status,
    Quarantine,
}

pub enum PMsg {
    DbUpdated(Result<(), String>),
    Scan(ScanOutcome),
    Quarantined(usize, Vec<String>),
    Startup(Vec<StartupItem>),
    StartupRemoved(String, Result<(), String>),
    Apps(Vec<AppCheck>),
    Exts(Vec<Ext>),
    Checks(Vec<Check>),
    QDone(K, usize, Vec<String>),
}

pub enum PConfirm {
    RemoveStartup(usize),
    DeleteQuarantine,
}

pub struct ProtectState {
    tab: PTab,
    clam: Option<protect::ClamStatus>,
    scan: Option<ScanOutcome>,
    scan_running: bool,
    ctl: Arc<ScanCtl>,
    total: Arc<AtomicU64>,
    startup: Vec<StartupItem>,
    startup_loaded: bool,
    apps: Vec<AppCheck>,
    apps_loaded: bool,
    exts: Vec<Ext>,
    exts_loaded: bool,
    checks: Vec<Check>,
    checks_loaded: bool,
    q: Vec<QItem>,
    q_loaded: bool,
    only_flagged: bool,
}

impl Default for ProtectState {
    fn default() -> Self {
        ProtectState {
            tab: PTab::Scan,
            clam: None,
            scan: None,
            scan_running: false,
            ctl: Arc::new(ScanCtl::default()),
            total: Arc::new(AtomicU64::new(0)),
            startup: Vec::new(),
            startup_loaded: false,
            apps: Vec::new(),
            apps_loaded: false,
            exts: Vec::new(),
            exts_loaded: false,
            checks: Vec::new(),
            checks_loaded: false,
            q: Vec::new(),
            q_loaded: false,
            only_flagged: false,
        }
    }
}

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(Color32::from_rgb(30, 30, 42))
        .corner_radius(10.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

fn flags_text(flags: &[K]) -> String {
    flags.iter().map(|k| t(*k)).collect::<Vec<_>>().join(" · ")
}

const WARN: Color32 = Color32::from_rgb(240, 170, 60);

impl App {
    pub(super) fn handle_protect(&mut self, m: PMsg) {
        match m {
            PMsg::DbUpdated(r) => {
                self.status = match r {
                    Ok(()) => t(K::DbUpdated).into(),
                    Err(e) => tf(K::DbUpdateFailed, &[&e]),
                };
                self.p.clam = Some(protect::clam_status());
            }
            PMsg::Scan(o) => {
                self.p.scan_running = false;
                self.status = if o.cancelled {
                    t(K::ScanCancelled).into()
                } else if let Some(e) = &o.error {
                    e.clone()
                } else if o.infected.is_empty() {
                    tf(K::ScanClean, &[&o.scanned.to_string()])
                } else {
                    tf(K::ScanFound, &[&o.infected.len().to_string(), &o.scanned.to_string()])
                };
                self.p.scan = Some(o);
            }
            PMsg::Quarantined(n, errs) => {
                self.status = tf(K::QuarantinedMsg, &[&n.to_string()]);
                self.errors = errs;
                if let Some(s) = self.p.scan.as_mut() {
                    s.infected.retain(|i| i.path.exists());
                }
                self.p.q_loaded = false;
            }
            PMsg::Startup(v) => {
                self.p.startup = v;
                self.p.startup_loaded = true;
            }
            PMsg::StartupRemoved(label, r) => {
                match r {
                    Ok(()) => self.status = tf(K::StartupRemovedMsg, &[&label]),
                    Err(e) => self.errors = vec![e],
                }
                self.p.startup_loaded = false;
            }
            PMsg::Apps(v) => {
                let flagged = v.iter().filter(|a| !a.flags.is_empty()).count();
                self.status = tf(K::AppsChecked, &[&v.len().to_string(), &flagged.to_string()]);
                self.p.apps = v;
                self.p.apps_loaded = true;
            }
            PMsg::Exts(v) => {
                self.status = tf(K::ExtCount, &[&v.len().to_string()]);
                self.p.exts = v;
                self.p.exts_loaded = true;
            }
            PMsg::Checks(v) => {
                self.p.checks = v;
                self.p.checks_loaded = true;
            }
            PMsg::QDone(k, n, errs) => {
                self.status = tf(k, &[&n.to_string()]);
                self.errors = errs;
                self.p.q_loaded = false;
            }
        }
    }

    pub(super) fn ui_protect(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        header(ui, t(K::NavProtect), t(K::ProtectSub));
        ui.horizontal_wrapped(|ui| {
            let tabs = [
                (PTab::Scan, "🐞", K::PTabScan),
                (PTab::Startup, "🚀", K::PTabStartup),
                (PTab::Apps, "🔏", K::PTabApps),
                (PTab::Ext, "🔌", K::PTabExt),
                (PTab::Status, "🛡", K::PTabStatus),
                (PTab::Quarantine, "🔒", K::PTabQuarantine),
            ];
            for (tab, icon, key) in tabs {
                let sel = self.p.tab == tab;
                let b = egui::Button::new(RichText::new(format!("{} {}", icon, t(key))).color(if sel {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(200, 200, 215)
                }))
                .fill(if sel { ACCENT } else { Color32::from_rgb(34, 34, 48) })
                .corner_radius(14.0);
                if ui.add(b).clicked() {
                    self.p.tab = tab;
                }
            }
        });
        ui.add_space(8.0);
        match self.p.tab {
            PTab::Scan => self.p_scan(ui, ctx),
            PTab::Startup => self.p_startup(ui, ctx),
            PTab::Apps => self.p_apps(ui, ctx),
            PTab::Ext => self.p_ext(ui, ctx),
            PTab::Status => self.p_status(ui, ctx),
            PTab::Quarantine => self.p_quarantine(ui, ctx),
        }
    }

    // ───────────── Ιοί ─────────────
    fn p_scan(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.p.scan_running {
            self.p_scan_progress(ui);
            return;
        }
        if self.busy_view(ui) {
            return;
        }
        if self.p.clam.is_none() {
            self.p.clam = Some(protect::clam_status());
        }
        let clam = self.p.clam.clone().unwrap();

        if clam.clamscan.is_none() {
            card(ui, |ui| {
                ui.label(RichText::new(t(K::ClamMissing)).strong().size(15.5));
                ui.add_space(6.0);
                if clam.has_brew {
                    if primary(ui, t(K::InstallClam)).clicked() {
                        protect::open_in_terminal("brew install clamav");
                    }
                    ui.label(RichText::new(t(K::InstallClamHint)).small().color(MUTED));
                } else {
                    ui.label(t(K::NeedBrew));
                    if primary(ui, t(K::OpenBrew)).clicked() {
                        protect::open_url("https://brew.sh");
                    }
                }
                ui.add_space(4.0);
                if ui.button(t(K::CheckAgain)).clicked() {
                    self.p.clam = Some(protect::clam_status());
                }
            });
            ui.add_space(10.0);
            ui.label(RichText::new(t(K::ClamNote)).small().color(MUTED));
            return;
        }

        let mut update = false;
        ui.horizontal(|ui| {
            if clam.db_files > 0 {
                let when = clam.db_updated.map(age_text).unwrap_or_else(|| t(K::Never).into());
                ui.label(RichText::new(tf(K::DbStatus, &[&clam.db_files.to_string(), &when])).color(MUTED));
            } else {
                ui.label(RichText::new(t(K::DbMissing)).color(DANGER));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(format!("⬇ {}", t(K::UpdateDb))).clicked() {
                    update = true;
                }
            });
        });
        if update {
            self.spawn(ctx, t(K::UpdateDb), |_| Msg::Protect(PMsg::DbUpdated(protect::update_db())));
            return;
        }
        ui.add_space(6.0);

        let ready = clam.db_files > 0;
        let mut start: Option<ScanKind> = None;
        let kinds = [
            (ScanKind::Quick, "⚡", K::QuickScan, K::QuickScanDesc),
            (ScanKind::Full, "🖴", K::FullScan, K::FullScanDesc),
            (ScanKind::Folder, "📁", K::FolderScan, K::FolderScanDesc),
        ];
        for (kind, icon, title, desc) in kinds {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon).size(24.0));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(t(title)).strong().size(15.5));
                        ui.label(RichText::new(t(desc)).small().color(MUTED));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add_enabled(ready, |ui: &mut egui::Ui| secondary(ui, t(K::Start), ACCENT)).clicked() {
                            start = Some(kind);
                        }
                    });
                });
            });
            ui.add_space(4.0);
        }
        if let Some(k) = start {
            self.start_virus_scan(ctx, k);
            return;
        }

        let mut quarantine: Vec<(PathBuf, String)> = Vec::new();
        if let Some(res) = self.p.scan.as_mut() {
            ui.add_space(8.0);
            let (txt, col) = if res.cancelled {
                (t(K::ScanCancelled).to_string(), MUTED)
            } else if let Some(e) = &res.error {
                (e.clone(), DANGER)
            } else if res.infected.is_empty() {
                (tf(K::ScanClean, &[&res.scanned.to_string()]), ACCENT2)
            } else {
                (tf(K::ScanFound, &[&res.infected.len().to_string(), &res.scanned.to_string()]), DANGER)
            };
            ui.label(RichText::new(txt).strong().size(16.0).color(col));
            if !res.infected.is_empty() {
                egui::ScrollArea::vertical().id_salt("inf").max_height(200.0).show(ui, |ui| {
                    for inf in res.infected.iter_mut() {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut inf.selected, "");
                            ui.label(RichText::new(&inf.threat).monospace().small().color(DANGER));
                            if ui
                                .add(egui::Label::new(RichText::new(tilde(&inf.path)).small()).truncate().sense(Sense::click()))
                                .on_hover_text(t(K::ClickReveal))
                                .clicked()
                            {
                                reveal_in_finder(&inf.path);
                            }
                        });
                    }
                });
                let any = res.infected.iter().any(|i| i.selected);
                if ui.add_enabled(any, |ui: &mut egui::Ui| secondary(ui, t(K::QuarantineSel), DANGER)).clicked() {
                    quarantine = res
                        .infected
                        .iter()
                        .filter(|i| i.selected)
                        .map(|i| (i.path.clone(), i.threat.clone()))
                        .collect();
                }
            }
        }
        if !quarantine.is_empty() {
            self.spawn(ctx, t(K::QuarantineSel), move |_| {
                let mut ok = 0;
                let mut errs = Vec::new();
                for (p, threat) in quarantine {
                    match protect::quarantine_one(&p, &threat) {
                        Ok(()) => ok += 1,
                        Err(e) => errs.push(e),
                    }
                }
                Msg::Protect(PMsg::Quarantined(ok, errs))
            });
        }
        ui.add_space(10.0);
        ui.label(RichText::new(t(K::ClamNote)).small().color(MUTED));
    }

    fn start_virus_scan(&mut self, ctx: &egui::Context, kind: ScanKind) {
        if self.busy.is_some() {
            return;
        }
        self.p.scan = None;
        self.p.scan_running = true;
        self.p.total.store(0, Ordering::Relaxed);
        self.p.ctl.cancel.store(false, Ordering::Relaxed);
        self.p.ctl.pid.store(0, Ordering::Relaxed);
        let total = self.p.total.clone();
        let ctl = self.p.ctl.clone();
        self.spawn(ctx, t(K::Scanning), move |c| {
            let targets = if kind == ScanKind::Folder {
                match protect::choose_folder(t(K::FolderScanDesc)) {
                    Some(p) => vec![p],
                    None => {
                        return Msg::Protect(PMsg::Scan(ScanOutcome { cancelled: true, ..Default::default() }));
                    }
                }
            } else {
                protect::scan_targets(kind)
            };
            Msg::Protect(PMsg::Scan(protect::scan(&targets, c, &total, &ctl)))
        });
    }

    fn p_scan_progress(&mut self, ui: &mut egui::Ui) {
        let total = self.p.total.load(Ordering::Relaxed);
        let done = self.counter.load(Ordering::Relaxed);
        ui.add_space(50.0);
        ui.vertical_centered(|ui| {
            ui.add(egui::Spinner::new().size(44.0).color(ACCENT));
            ui.add_space(10.0);
            ui.label(RichText::new(format!("{}…", t(K::Scanning))).size(18.0));
            ui.add_space(6.0);
            if total == 0 {
                ui.label(RichText::new(t(K::Preparing)).color(MUTED));
            } else if done == 0 {
                ui.label(RichText::new(t(K::LoadingDb)).color(MUTED));
            } else {
                ui.add(
                    egui::ProgressBar::new((done as f32 / total as f32).min(1.0))
                        .desired_width(440.0)
                        .fill(ACCENT)
                        .show_percentage(),
                );
                ui.label(
                    RichText::new(tf(K::ScanProgress, &[&done.to_string(), &total.to_string()])).color(MUTED),
                );
            }
            ui.add_space(14.0);
            if secondary(ui, t(K::Stop), DANGER).clicked() {
                self.p.ctl.stop();
            }
        });
    }

    // ───────────── Αυτόματη εκκίνηση ─────────────
    fn p_startup(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(RichText::new(t(K::StartupSub)).color(MUTED));
        if self.busy_view(ui) {
            return;
        }
        if !self.p.startup_loaded {
            self.spawn(ctx, t(K::Checking), |_| Msg::Protect(PMsg::Startup(protect::startup_items())));
            return;
        }
        let mut reload = false;
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.p.only_flagged, t(K::OnlySuspicious));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                reload = ui.button(t(K::CheckAgain)).clicked();
            });
        });
        if reload {
            self.p.startup_loaded = false;
            return;
        }
        ui.separator();
        let shown: Vec<usize> = (0..self.p.startup.len())
            .filter(|&i| !self.p.only_flagged || !self.p.startup[i].flags.is_empty())
            .collect();
        if shown.iter().all(|&i| self.p.startup[i].flags.is_empty()) {
            ui.label(RichText::new(t(K::NoneFound)).color(ACCENT2));
        }
        let mut remove = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for &i in &shown {
                let it = &self.p.startup[i];
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (icon, col) = if it.flags.is_empty() { ("✔", ACCENT2) } else { ("⚠", DANGER) };
                        ui.label(RichText::new(icon).size(20.0).color(col));
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&it.label).strong());
                                let scope = if it.system { t(K::ScopeSystem) } else { t(K::ScopeUser) };
                                ui.label(RichText::new(scope).small().color(MUTED));
                            });
                            ui.add(egui::Label::new(RichText::new(&it.command).small().monospace()).truncate());
                            // Αν η υπογραφή είναι ήδη πρόβλημα (π.χ. «λείπει»), δεν τη γράφουμε δύο φορές.
                            let line = if it.flags.is_empty() {
                                it.sig.label()
                            } else if it.sig.flag().is_some() {
                                flags_text(&it.flags)
                            } else {
                                format!("{}  ·  {}", it.sig.label(), flags_text(&it.flags))
                            };
                            ui.label(RichText::new(line).small().color(if it.flags.is_empty() { MUTED } else { DANGER }));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(t(K::Remove)).clicked() {
                                remove = Some(i);
                            }
                            if ui.small_button("🔎").on_hover_text(t(K::ShowInFinder)).clicked() {
                                reveal_in_finder(&it.plist);
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
        });
        if let Some(i) = remove {
            self.confirm = Some(Confirm::Protect(PConfirm::RemoveStartup(i)));
        }
    }

    // ───────────── Υπογραφές εφαρμογών ─────────────
    fn p_apps(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(RichText::new(t(K::AppsSigSub)).color(MUTED));
        if self.busy_view(ui) {
            return;
        }
        if !self.p.apps_loaded {
            self.spawn(ctx, t(K::Checking), |c| Msg::Protect(PMsg::Apps(protect::check_apps(c))));
            return;
        }
        let mut reload = false;
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.p.only_flagged, t(K::OnlySuspicious));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                reload = ui.button(t(K::CheckAgain)).clicked();
            });
        });
        if reload {
            self.p.apps_loaded = false;
            return;
        }
        ui.separator();
        let only = self.p.only_flagged;
        let shown: Vec<&AppCheck> = self.p.apps.iter().filter(|a| !only || !a.flags.is_empty()).collect();
        if shown.iter().all(|a| a.flags.is_empty()) {
            ui.label(RichText::new(t(K::NoneFound)).color(ACCENT2));
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for a in shown {
                ui.horizontal(|ui| {
                    let (icon, col) = if a.flags.is_empty() { ("✔", ACCENT2) } else { ("⚠", WARN) };
                    ui.label(RichText::new(icon).color(col));
                    ui.add_sized([220.0, 20.0], egui::Label::new(RichText::new(&a.name).strong()).truncate());
                    ui.add_sized([260.0, 20.0], egui::Label::new(RichText::new(a.sig.label()).small().color(MUTED)).truncate());
                    ui.label(RichText::new(a.gk.label()).small().color(if a.flags.is_empty() { MUTED } else { WARN }));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("🔎").on_hover_text(t(K::ShowInFinder)).clicked() {
                            reveal_in_finder(&a.path);
                        }
                    });
                });
            }
        });
    }

    // ───────────── Επεκτάσεις ─────────────
    fn p_ext(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(RichText::new(t(K::ExtSub)).color(MUTED));
        if self.busy_view(ui) {
            return;
        }
        if !self.p.exts_loaded {
            self.spawn(ctx, t(K::Checking), |_| Msg::Protect(PMsg::Exts(protect::browser_extensions())));
            return;
        }
        let mut reload = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(t(K::ExtRemoveHint)).small());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                reload = ui.button(t(K::CheckAgain)).clicked();
            });
        });
        if reload {
            self.p.exts_loaded = false;
            return;
        }
        ui.separator();
        if self.p.exts.is_empty() {
            ui.label(RichText::new(t(K::NoneFound)).color(ACCENT2));
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for e in &self.p.exts {
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        let risky = e.all_sites || !e.sensitive.is_empty();
                        ui.label(RichText::new(if risky { "⚠" } else { "🔌" }).size(18.0).color(if risky { WARN } else { MUTED }));
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&e.name).strong());
                                ui.label(RichText::new(&e.version).small().color(MUTED));
                                if e.enabled == Some(false) {
                                    ui.label(RichText::new(t(K::Disabled)).small().color(MUTED));
                                }
                            });
                            ui.label(RichText::new(format!("{}  ·  {}", e.browser, e.id)).small().color(MUTED));
                            if e.all_sites {
                                ui.label(RichText::new(t(K::ExtAllSites)).small().color(WARN));
                            }
                            if !e.sensitive.is_empty() {
                                ui.label(RichText::new(tf(K::ExtSensitive, &[&e.sensitive.join(", ")])).small().color(WARN));
                            }
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("🔎").on_hover_text(t(K::ShowInFinder)).clicked() {
                                reveal_in_finder(&e.path);
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
        });
    }

    // ───────────── Κατάσταση ασφαλείας ─────────────
    fn p_status(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(RichText::new(t(K::StatusSub)).color(MUTED));
        if self.busy_view(ui) {
            return;
        }
        if !self.p.checks_loaded {
            self.spawn(ctx, t(K::Checking), |_| Msg::Protect(PMsg::Checks(protect::security_checks())));
            return;
        }
        let good = self.p.checks.iter().filter(|c| c.level == Level::Good).count();
        let mut reload = false;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tf(K::StatusSummary, &[&good.to_string(), &self.p.checks.len().to_string()]))
                    .strong()
                    .size(16.0),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                reload = ui.button(t(K::CheckAgain)).clicked();
            });
        });
        if reload {
            self.p.checks_loaded = false;
            return;
        }
        ui.add_space(6.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for c in &self.p.checks {
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (icon, col) = match c.level {
                            Level::Good => ("✔", ACCENT2),
                            Level::Warn => ("⚠", DANGER),
                            Level::Unknown => ("?", MUTED),
                        };
                        ui.label(RichText::new(icon).size(20.0).color(col));
                        ui.vertical(|ui| {
                            ui.label(RichText::new(t(c.name)).strong());
                            ui.label(RichText::new(&c.detail).small().color(MUTED));
                            if c.level == Level::Warn {
                                ui.label(RichText::new(t(c.warn)).small().color(DANGER));
                            }
                        });
                        if c.level == Level::Warn {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button(t(K::OpenSettings)).clicked() {
                                    protect::open_url(c.url);
                                }
                            });
                        }
                    });
                });
                ui.add_space(4.0);
            }
        });
    }

    // ───────────── Καραντίνα ─────────────
    fn p_quarantine(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(RichText::new(t(K::QSub)).color(MUTED));
        if self.busy_view(ui) {
            return;
        }
        if !self.p.q_loaded {
            self.p.q = protect::list_quarantine();
            self.p.q_loaded = true;
        }
        if self.p.q.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("🔒").size(48.0));
                ui.label(RichText::new(t(K::QEmpty)).color(MUTED));
            });
            return;
        }
        let any = self.p.q.iter().any(|q| q.selected);
        let mut restore = false;
        ui.horizontal(|ui| {
            if ui.small_button(t(K::SelectAll)).clicked() {
                self.p.q.iter_mut().for_each(|q| q.selected = true);
            }
            if ui.small_button(t(K::SelectNone)).clicked() {
                self.p.q.iter_mut().for_each(|q| q.selected = false);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add_enabled(any, |ui: &mut egui::Ui| secondary(ui, t(K::DeleteForever), DANGER)).clicked() {
                    self.confirm = Some(Confirm::Protect(PConfirm::DeleteQuarantine));
                }
                if ui.add_enabled(any, egui::Button::new(t(K::Restore))).clicked() {
                    restore = true;
                }
            });
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for q in self.p.q.iter_mut() {
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut q.selected, "");
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(file_name(&q.file)).strong());
                                ui.label(RichText::new(human(q.size)).small().color(MUTED));
                                ui.label(RichText::new(age_text(q.when)).small().color(MUTED));
                            });
                            ui.label(RichText::new(&q.threat).small().monospace().color(DANGER));
                            ui.add(
                                egui::Label::new(RichText::new(tf(K::QFrom, &[&tilde(&q.original)])).small().color(MUTED))
                                    .truncate(),
                            );
                        });
                    });
                });
                ui.add_space(4.0);
            }
        });
        if restore {
            let items: Vec<QItem> = self.p.q.iter().filter(|q| q.selected).cloned().collect();
            self.spawn(ctx, t(K::Restore), move |_| {
                let (ok, errs) = apply(&items, protect::restore);
                Msg::Protect(PMsg::QDone(K::RestoredMsg, ok, errs))
            });
        }
    }

    // ───────────── Επιβεβαιώσεις ─────────────
    pub(super) fn protect_confirm_text(&self, c: &PConfirm) -> (String, String, &'static str) {
        match c {
            PConfirm::RemoveStartup(i) => {
                let label = self.p.startup.get(*i).map(|s| s.label.clone()).unwrap_or_default();
                (t(K::Remove).to_string(), tf(K::RemovePersistBody, &[&label]), t(K::Remove))
            }
            PConfirm::DeleteQuarantine => {
                let n = self.p.q.iter().filter(|q| q.selected).count();
                (t(K::DeleteForever).to_string(), tf(K::DeleteQBody, &[&n.to_string()]), t(K::DeleteForever))
            }
        }
    }

    pub(super) fn execute_protect(&mut self, c: PConfirm, ctx: &egui::Context) {
        match c {
            PConfirm::RemoveStartup(i) => {
                let Some(item) = self.p.startup.get(i).cloned() else { return };
                self.spawn(ctx, t(K::Remove), move |_| {
                    let r = protect::remove_startup(&item);
                    Msg::Protect(PMsg::StartupRemoved(item.label.clone(), r))
                });
            }
            PConfirm::DeleteQuarantine => {
                let items: Vec<QItem> = self.p.q.iter().filter(|q| q.selected).cloned().collect();
                self.spawn(ctx, t(K::DeleteForever), move |_| {
                    let (ok, errs) = apply(&items, protect::delete_quarantined);
                    Msg::Protect(PMsg::QDone(K::DeletedMsg, ok, errs))
                });
            }
        }
    }
}

fn apply(items: &[QItem], f: fn(&QItem) -> Result<(), String>) -> (usize, Vec<String>) {
    let mut ok = 0;
    let mut errs = Vec::new();
    for it in items {
        match f(it) {
            Ok(()) => ok += 1,
            Err(e) => errs.push(e),
        }
    }
    (ok, errs)
}
