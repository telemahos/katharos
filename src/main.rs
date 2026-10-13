//! Katharos — καθαριστής δίσκου για macOS, γραμμένος σε Rust.

mod apps;
mod files;
mod i18n;
mod junk;
mod lens;
mod maint;
mod util;
#[cfg(test)]
mod tests;

use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, Vec2};
use files::{DupGroup, FileInfo};
use i18n::{t, tf, K};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;
use util::*;

const ACCENT: Color32 = Color32::from_rgb(124, 92, 255);
const ACCENT2: Color32 = Color32::from_rgb(0, 200, 170);
const DANGER: Color32 = Color32::from_rgb(235, 87, 87);
const MUTED: Color32 = Color32::from_rgb(150, 150, 170);
const ROW_LIMIT: usize = 400;

fn main() -> eframe::Result<()> {
    // Η γλώσσα βρίσκεται από τον Mac. Για δοκιμή: KATHAROS_LANG=de katharos
    let lang = std::env::var("KATHAROS_LANG")
        .ok()
        .and_then(|s| i18n::from_code(&s))
        .unwrap_or_else(i18n::detect);
    i18n::set_lang(lang);

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--scan") {
        cli_scan();
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 740.0])
            .with_min_inner_size([920.0, 600.0])
            .with_title("Katharos"),
        ..Default::default()
    };
    eframe::run_native("Katharos", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

/// `katharos --scan`: γρήγορη σάρωση στο τερματικό, χωρίς να σβήσει τίποτα.
fn cli_scan() {
    let c = AtomicU64::new(0);
    let start = std::time::Instant::now();
    let cats = junk::scan(&c);
    let mut total = 0;
    for cat in &cats {
        println!(
            "{:<45} {:>10}  ({})",
            t(cat.name),
            human(cat.total()),
            tf(K::ItemsCount, &[&cat.items.len().to_string()])
        );
        for it in cat.items.iter().take(3) {
            println!("    {:<60} {:>10}", tilde(&it.path), human(it.size));
        }
        total += cat.selected_size();
    }
    println!(
        "\n{}",
        tf(
            K::CliSummary,
            &[
                &human(total),
                &c.load(Ordering::Relaxed).to_string(),
                &format!("{:.2}", start.elapsed().as_secs_f32()),
            ]
        )
    );
}

#[derive(PartialEq, Clone, Copy)]
enum View {
    Smart,
    Large,
    Dupes,
    Uninstall,
    Lens,
    Maint,
}

enum Msg {
    Junk(Vec<junk::Category>),
    Cleaned(junk::CleanResult),
    Large(Vec<FileInfo>),
    Dupes(Vec<DupGroup>),
    Apps(Vec<apps::AppInfo>),
    Leftovers(String, Vec<junk::Item>),
    Lens(PathBuf, Vec<lens::Entry>),
    Removed { ok: Vec<PathBuf>, bytes: u64, errors: Vec<String> },
    Maint(String),
}

enum Confirm {
    CleanJunk,
    TrashLarge,
    TrashDupes,
    Uninstall,
    TrashLens(PathBuf, u64),
}

struct App {
    view: View,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    counter: Arc<AtomicU64>,
    busy: Option<String>,
    status: String,
    errors: Vec<String>,
    disk: (u64, u64),
    confirm: Option<Confirm>,

    junk: Vec<junk::Category>,
    junk_scanned: bool,
    freed_session: u64,

    large: Vec<FileInfo>,
    large_scanned: bool,
    large_min_mb: u64,
    large_age: u32,
    large_sel: HashSet<PathBuf>,

    dupes: Vec<DupGroup>,
    dupes_scanned: bool,
    dupe_sel: HashSet<PathBuf>,

    apps: Vec<apps::AppInfo>,
    apps_loaded: bool,
    app_filter: String,
    app_sel: Option<usize>,
    leftovers: Vec<junk::Item>,

    lens_path: PathBuf,
    lens: Vec<lens::Entry>,
    lens_loaded_for: Option<PathBuf>,

    maint_log: Vec<String>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_style(&cc.egui_ctx);
        let (tx, rx) = channel();
        App {
            view: View::Smart,
            tx,
            rx,
            counter: Arc::new(AtomicU64::new(0)),
            busy: None,
            status: String::new(),
            errors: Vec::new(),
            disk: disk_space(),
            confirm: None,
            junk: Vec::new(),
            junk_scanned: false,
            freed_session: 0,
            large: Vec::new(),
            large_scanned: false,
            large_min_mb: 100,
            large_age: 0,
            large_sel: HashSet::new(),
            dupes: Vec::new(),
            dupes_scanned: false,
            dupe_sel: HashSet::new(),
            apps: Vec::new(),
            apps_loaded: false,
            app_filter: String::new(),
            app_sel: None,
            leftovers: Vec::new(),
            lens_path: home(),
            lens: Vec::new(),
            lens_loaded_for: None,
            maint_log: Vec::new(),
        }
    }

    fn spawn<F>(&mut self, ctx: &egui::Context, label: &str, f: F)
    where
        F: FnOnce(&AtomicU64) -> Msg + Send + 'static,
    {
        if self.busy.is_some() {
            return;
        }
        self.busy = Some(label.to_string());
        self.errors.clear();
        self.counter.store(0, Ordering::Relaxed);
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let counter = self.counter.clone();
        std::thread::spawn(move || {
            let m = f(&counter);
            let _ = tx.send(m);
            ctx.request_repaint();
        });
    }

    fn handle_messages(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            self.busy = None;
            match m {
                Msg::Junk(c) => {
                    let n: u64 = c.iter().map(|x| x.total()).sum();
                    self.status = tf(K::FoundSummary, &[&human(n), &c.len().to_string()]);
                    self.junk = c;
                    self.junk_scanned = true;
                }
                Msg::Cleaned(r) => {
                    self.freed_session += r.freed;
                    let mut s = tf(K::Freed, &[&human(r.freed)]);
                    if r.trashed > 0 {
                        s += &tf(K::TrashedSuffix, &[&human(r.trashed)]);
                    }
                    self.status = s;
                    self.errors = r.errors;
                    self.disk = disk_space();
                    let errs = std::mem::take(&mut self.errors);
                    self.spawn(ctx, t(K::Rechecking), |c| Msg::Junk(junk::scan(c)));
                    self.errors = errs;
                }
                Msg::Large(v) => {
                    self.status = tf(K::LargeFound, &[&v.len().to_string()]);
                    self.large = v;
                    self.large_scanned = true;
                    self.large_sel.clear();
                }
                Msg::Dupes(g) => {
                    let w: u64 = g.iter().map(|x| x.wasted()).sum();
                    self.status = tf(K::DupeSummary, &[&g.len().to_string(), &human(w)]);
                    self.dupe_sel = g
                        .iter()
                        .flat_map(|x| x.files.iter().skip(1).map(|f| f.path.clone()))
                        .collect();
                    self.dupes = g;
                    self.dupes_scanned = true;
                }
                Msg::Apps(a) => {
                    self.status = tf(K::AppsCount, &[&a.len().to_string()]);
                    self.apps = a;
                    self.apps_loaded = true;
                    self.app_sel = None;
                    self.leftovers.clear();
                }
                Msg::Leftovers(id, items) => {
                    if self.app_sel.and_then(|i| self.apps.get(i)).map(|a| a.id == id).unwrap_or(false) {
                        self.leftovers = items;
                    }
                }
                Msg::Lens(p, e) => {
                    self.lens_loaded_for = Some(p);
                    self.lens = e;
                }
                Msg::Removed { ok, bytes, errors } => {
                    let set: HashSet<PathBuf> = ok.iter().cloned().collect();
                    self.status = tf(K::MovedToTrash, &[&ok.len().to_string(), &human(bytes)]);
                    self.errors = errors;
                    self.large.retain(|f| !set.contains(&f.path));
                    self.large_sel.retain(|p| !set.contains(p));
                    for g in &mut self.dupes {
                        g.files.retain(|f| !set.contains(&f.path));
                    }
                    self.dupes.retain(|g| g.files.len() > 1);
                    self.dupe_sel.retain(|p| !set.contains(p));
                    self.lens.retain(|e| !set.contains(&e.path));
                    if let Some(i) = self.app_sel {
                        if self.apps.get(i).map(|a| set.contains(&a.path)).unwrap_or(false) {
                            self.apps.remove(i);
                            self.app_sel = None;
                            self.leftovers.clear();
                        }
                    }
                    self.disk = disk_space();
                }
                Msg::Maint(s) => self.maint_log.push(s),
            }
        }
    }
}

/// Φορτώνει γραμματοσειρά με κινεζικά γράμματα (το macOS έχει το PingFang).
fn install_fonts(ctx: &egui::Context) {
    let candidates = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    ];
    if let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) {
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("cjk".to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
        for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(fam).or_default().push("cjk".to_owned());
        }
        ctx.set_fonts(fonts);
    }
}

fn setup_style(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(22, 22, 30);
    v.window_fill = Color32::from_rgb(30, 30, 42);
    v.extreme_bg_color = Color32::from_rgb(16, 16, 22);
    v.faint_bg_color = Color32::from_rgb(28, 28, 38);
    v.selection.bg_fill = ACCENT;
    v.hyperlink_color = ACCENT2;
    v.widgets.inactive.corner_radius = 8.0.into();
    v.widgets.hovered.corner_radius = 8.0.into();
    v.widgets.active.corner_radius = 8.0.into();
    v.window_corner_radius = 14.0.into();
    ctx.set_visuals(v);
    let mut st = (*ctx.style()).clone();
    use egui::{FontId, TextStyle};
    st.text_styles.insert(TextStyle::Body, FontId::proportional(14.5));
    st.text_styles.insert(TextStyle::Button, FontId::proportional(14.5));
    st.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
    st.text_styles.insert(TextStyle::Heading, FontId::proportional(26.0));
    st.spacing.item_spacing = Vec2::new(8.0, 7.0);
    st.spacing.button_padding = Vec2::new(12.0, 6.0);
    ctx.set_style(st);
}

fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(17.0).strong().color(Color32::WHITE))
            .fill(ACCENT)
            .min_size(Vec2::new(230.0, 46.0))
            .corner_radius(23.0),
    )
}

fn secondary(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).strong().color(Color32::WHITE))
            .fill(color)
            .min_size(Vec2::new(0.0, 34.0))
            .corner_radius(17.0),
    )
}

fn header(ui: &mut egui::Ui, title: &str, sub: &str) {
    ui.add_space(6.0);
    ui.heading(RichText::new(title).strong());
    ui.label(RichText::new(sub).color(MUTED));
    ui.add_space(10.0);
}

fn size_label(ui: &mut egui::Ui, b: u64) {
    ui.label(RichText::new(human(b)).monospace().strong());
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages(ctx);
        if self.busy.is_some() {
            ctx.request_repaint_after(Duration::from_millis(120));
        }

        egui::SidePanel::left("nav")
            .exact_width(230.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(Color32::from_rgb(17, 17, 24)).inner_margin(16.0))
            .show(ctx, |ui| self.sidebar(ui));

        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::new().fill(Color32::from_rgb(17, 17, 24)).inner_margin(egui::Margin::symmetric(16, 8)))
            .show(ctx, |ui| self.status_bar(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(22, 22, 30)).inner_margin(24.0))
            .show(ctx, |ui| match self.view {
                View::Smart => self.ui_smart(ui, ctx),
                View::Large => self.ui_large(ui, ctx),
                View::Dupes => self.ui_dupes(ui, ctx),
                View::Uninstall => self.ui_uninstall(ui, ctx),
                View::Lens => self.ui_lens(ui, ctx),
                View::Maint => self.ui_maint(ui, ctx),
            });

        self.confirm_dialog(ctx);
    }
}

impl App {
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("✨ Katharos").size(24.0).strong().color(Color32::WHITE));
        ui.label(RichText::new(t(K::Tagline)).color(MUTED).small());
        ui.add_space(18.0);
        let items = [
            (View::Smart, "♻", K::NavSmart),
            (View::Large, "📦", K::NavLarge),
            (View::Dupes, "📑", K::NavDupes),
            (View::Uninstall, "🗑", K::NavUninstall),
            (View::Lens, "🔍", K::NavLens),
            (View::Maint, "⚙", K::NavMaint),
        ];
        for (v, icon, key) in items {
            let selected = self.view == v;
            let label = format!("{}  {}", icon, t(key));
            let btn = egui::Button::new(RichText::new(label).size(15.5).color(if selected {
                Color32::WHITE
            } else {
                Color32::from_rgb(200, 200, 215)
            }))
            .fill(if selected { ACCENT } else { Color32::TRANSPARENT })
            .min_size(Vec2::new(ui.available_width(), 38.0))
            .corner_radius(10.0);
            if ui.add(btn).clicked() {
                self.view = v;
            }
        }
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            // Στο κάτω μέρος: επιλογή γλώσσας (η πρώτη γραμμή μπαίνει κάτω-κάτω)
            let current = i18n::lang();
            egui::ComboBox::from_id_salt("lang")
                .selected_text(current.name())
                .width(190.0)
                .show_ui(ui, |ui| {
                    for l in i18n::ALL {
                        if ui.selectable_label(current == l, l.name()).clicked() && current != l {
                            i18n::set_lang(l);
                            self.status.clear();
                        }
                    }
                });
            ui.label(RichText::new(t(K::Language)).small().color(MUTED));
            ui.add_space(10.0);

            let (total, free) = self.disk;
            if total > 0 {
                if self.freed_session > 0 {
                    ui.label(
                        RichText::new(tf(K::FreedToday, &[&human(self.freed_session)]))
                            .color(ACCENT2)
                            .small(),
                    );
                }
                let used = total.saturating_sub(free);
                ui.label(RichText::new(tf(K::FreeOf, &[&human(free), &human(total)])).small().color(MUTED));
                ui.add(
                    egui::ProgressBar::new(used as f32 / total as f32)
                        .fill(if free < total / 10 { DANGER } else { ACCENT })
                        .desired_height(8.0),
                );
                ui.label(RichText::new(t(K::Disk)).strong());
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(b) = &self.busy {
                ui.spinner();
                ui.label(tf(K::Busy, &[b, &self.counter.load(Ordering::Relaxed).to_string()]));
            } else {
                ui.label(RichText::new(&self.status).color(MUTED));
            }
            if !self.errors.is_empty() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button(
                        RichText::new(tf(K::ErrorsCount, &[&self.errors.len().to_string()])).color(DANGER),
                        |ui| {
                            ui.label(RichText::new(t(K::ErrorsHelp)).small());
                            ui.separator();
                            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                                for e in self.errors.iter().take(200) {
                                    ui.label(RichText::new(e).small().monospace());
                                }
                            });
                        },
                    );
                });
            }
        });
    }

    fn busy_view(&self, ui: &mut egui::Ui) -> bool {
        if let Some(b) = &self.busy {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                ui.add(egui::Spinner::new().size(48.0).color(ACCENT));
                ui.add_space(12.0);
                ui.label(RichText::new(format!("{}…", b)).size(18.0));
                ui.label(
                    RichText::new(tf(K::ItemsChecked, &[&self.counter.load(Ordering::Relaxed).to_string()]))
                        .color(MUTED),
                );
            });
            true
        } else {
            false
        }
    }

    fn start_screen(&mut self, ui: &mut egui::Ui, icon: &str, text: &str, button: &str) -> bool {
        let mut clicked = false;
        ui.add_space(70.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(icon).size(64.0));
            ui.add_space(8.0);
            ui.label(RichText::new(text).size(16.0).color(MUTED));
            ui.add_space(20.0);
            clicked = primary(ui, button).clicked();
        });
        clicked
    }

    // ───────────────────────── Έξυπνη Σάρωση ─────────────────────────
    fn ui_smart(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        header(ui, t(K::NavSmart), t(K::SmartSub));
        if self.busy_view(ui) {
            return;
        }
        if !self.junk_scanned {
            if self.start_screen(ui, "♻", t(K::SmartIntro), t(K::Scan)) {
                self.spawn(ctx, t(K::Scan), |c| Msg::Junk(junk::scan(c)));
            }
            return;
        }
        let sel: u64 = self.junk.iter().map(|c| c.selected_size()).sum();
        let total: u64 = self.junk.iter().map(|c| c.total()).sum();

        ui.horizontal(|ui| {
            ui.label(RichText::new(human(sel)).size(40.0).strong().color(ACCENT2));
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(t(K::SelectedForCleaning)).color(MUTED));
                ui.label(RichText::new(tf(K::OfFound, &[&human(total)])).small().color(MUTED));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add_enabled(sel > 0, |ui: &mut egui::Ui| primary(ui, t(K::Clean))).clicked() {
                    self.confirm = Some(Confirm::CleanJunk);
                }
                if ui.button(t(K::Rescan)).clicked() {
                    self.spawn(ctx, t(K::Scan), |c| Msg::Junk(junk::scan(c)));
                }
            });
        });
        ui.add_space(10.0);
        if self.junk.is_empty() {
            ui.label(t(K::MacClean));
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (ci, cat) in self.junk.iter_mut().enumerate() {
                egui::Frame::new()
                    .fill(Color32::from_rgb(30, 30, 42))
                    .corner_radius(12.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let mut all = cat.items.iter().all(|i| i.selected);
                            if ui.checkbox(&mut all, "").changed() {
                                cat.set_all(all);
                            }
                            ui.vertical(|ui| {
                                ui.label(RichText::new(t(cat.name)).strong().size(15.5));
                                ui.label(RichText::new(t(cat.desc)).small().color(MUTED));
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(human(cat.selected_size())).strong().size(16.0));
                            });
                        });
                        egui::CollapsingHeader::new(
                            RichText::new(tf(
                                K::ItemsSelectedOf,
                                &[&cat.selected_count().to_string(), &cat.items.len().to_string()],
                            ))
                            .small()
                            .color(MUTED),
                        )
                        .id_salt(("cat", ci))
                        .show(ui, |ui| {
                            for it in cat.items.iter_mut().take(ROW_LIMIT) {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut it.selected, "");
                                    let r = ui.add(
                                        egui::Label::new(RichText::new(tilde(&it.path)).small()).truncate().sense(Sense::click()),
                                    );
                                    if r.on_hover_text(t(K::ClickReveal)).clicked() {
                                        reveal_in_finder(&it.path);
                                    }
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(RichText::new(human(it.size)).small().monospace());
                                    });
                                });
                            }
                            if cat.items.len() > ROW_LIMIT {
                                ui.label(
                                    RichText::new(tf(K::AndMore, &[&(cat.items.len() - ROW_LIMIT).to_string()]))
                                        .small()
                                        .color(MUTED),
                                );
                            }
                        });
                    });
                ui.add_space(6.0);
            }
        });
    }

    // ───────────────────────── Μεγάλα & Παλιά ─────────────────────────
    fn ui_large(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        header(ui, t(K::LargeTitle), t(K::LargeSub));
        if self.busy_view(ui) {
            return;
        }
        if !self.large_scanned {
            if self.start_screen(ui, "📦", t(K::LargeIntro), t(K::Scan)) {
                self.spawn(ctx, t(K::LargeSearching), |c| Msg::Large(files::scan_large(10_000_000, c)));
            }
            return;
        }
        ui.horizontal(|ui| {
            ui.label(t(K::MinSize));
            ui.add(
                egui::Slider::new(&mut self.large_min_mb, 10..=10_000)
                    .logarithmic(true)
                    .suffix(" MB"),
            );
            ui.add_space(12.0);
            ui.label(t(K::Unused));
            egui::ComboBox::from_id_salt("age")
                .selected_text(match self.large_age {
                    0 => t(K::AgeAll),
                    90 => t(K::Age3m),
                    180 => t(K::Age6m),
                    _ => t(K::Age1y),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.large_age, 0, t(K::AgeAll));
                    ui.selectable_value(&mut self.large_age, 90, t(K::Age3m));
                    ui.selectable_value(&mut self.large_age, 180, t(K::Age6m));
                    ui.selectable_value(&mut self.large_age, 365, t(K::Age1y));
                });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(t(K::Rescan)).clicked() {
                    self.spawn(ctx, t(K::LargeSearching), |c| Msg::Large(files::scan_large(10_000_000, c)));
                }
            });
        });
        let min = self.large_min_mb * 1_000_000;
        let age = self.large_age as u64;
        let shown: Vec<usize> = self
            .large
            .iter()
            .enumerate()
            .filter(|(_, f)| f.size >= min && (age == 0 || age_days(f.last_used) >= age))
            .map(|(i, _)| i)
            .collect();
        let sel_size: u64 = self.large.iter().filter(|f| self.large_sel.contains(&f.path)).map(|f| f.size).sum();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tf(
                    K::LargeSel,
                    &[&shown.len().to_string(), &self.large_sel.len().to_string(), &human(sel_size)],
                ))
                .color(MUTED),
            );
            if ui.small_button(t(K::SelectAll)).clicked() {
                for &i in &shown {
                    self.large_sel.insert(self.large[i].path.clone());
                }
            }
            if ui.small_button(t(K::SelectNone)).clicked() {
                self.large_sel.clear();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(!self.large_sel.is_empty(), |ui: &mut egui::Ui| secondary(ui, t(K::MoveToTrash), DANGER))
                    .clicked()
                {
                    self.confirm = Some(Confirm::TrashLarge);
                }
            });
        });
        ui.separator();
        let row_h = 26.0;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row_h, shown.len(), |ui, range| {
            for &i in &shown[range] {
                let f = &self.large[i];
                ui.horizontal(|ui| {
                    ui.set_height(row_h - 4.0);
                    let mut on = self.large_sel.contains(&f.path);
                    if ui.checkbox(&mut on, "").changed() {
                        if on {
                            self.large_sel.insert(f.path.clone());
                        } else {
                            self.large_sel.remove(&f.path);
                        }
                    }
                    ui.add_sized([90.0, row_h - 4.0], egui::Label::new(RichText::new(human(f.size)).monospace().strong()));
                    ui.add_sized([130.0, row_h - 4.0], egui::Label::new(RichText::new(age_text(f.last_used)).small().color(MUTED)));
                    if ui.small_button("🔎").on_hover_text(t(K::ShowInFinder)).clicked() {
                        reveal_in_finder(&f.path);
                    }
                    ui.add(egui::Label::new(file_name(&f.path)).truncate())
                        .on_hover_text(tilde(&f.path));
                });
            }
        });
    }

    // ───────────────────────── Διπλότυπα ─────────────────────────
    fn ui_dupes(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        header(ui, t(K::NavDupes), t(K::DupesSub));
        if self.busy_view(ui) {
            return;
        }
        if !self.dupes_scanned {
            if self.start_screen(ui, "📑", t(K::DupesIntro), t(K::FindDupes)) {
                self.spawn(ctx, t(K::Comparing), |c| Msg::Dupes(files::scan_dupes(256 * 1024, c)));
            }
            return;
        }
        let sel_size: u64 = self
            .dupes
            .iter()
            .flat_map(|g| g.files.iter())
            .filter(|f| self.dupe_sel.contains(&f.path))
            .map(|f| f.size)
            .sum();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tf(
                    K::DupesSel,
                    &[&self.dupes.len().to_string(), &self.dupe_sel.len().to_string(), &human(sel_size)],
                ))
                .color(MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(!self.dupe_sel.is_empty(), |ui: &mut egui::Ui| secondary(ui, t(K::MoveToTrash), DANGER))
                    .clicked()
                {
                    self.confirm = Some(Confirm::TrashDupes);
                }
                if ui.button(t(K::Rescan)).clicked() {
                    self.spawn(ctx, t(K::Comparing), |c| Msg::Dupes(files::scan_dupes(256 * 1024, c)));
                }
            });
        });
        ui.separator();
        if self.dupes.is_empty() {
            ui.label(t(K::NoDupes));
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for g in self.dupes.iter().take(ROW_LIMIT) {
                egui::Frame::new()
                    .fill(Color32::from_rgb(30, 30, 42))
                    .corner_radius(10.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(tf(
                                K::DupGroupLine,
                                &[
                                    &file_name(&g.files[0].path),
                                    &g.files.len().to_string(),
                                    &human(g.size),
                                    &human(g.wasted()),
                                ],
                            ))
                            .strong(),
                        );
                        for (fi, f) in g.files.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let mut on = self.dupe_sel.contains(&f.path);
                                if ui.checkbox(&mut on, "").changed() {
                                    if on {
                                        self.dupe_sel.insert(f.path.clone());
                                    } else {
                                        self.dupe_sel.remove(&f.path);
                                    }
                                }
                                if fi == 0 {
                                    ui.label(RichText::new(t(K::Original)).small().color(ACCENT2));
                                }
                                if ui
                                    .add(egui::Label::new(RichText::new(tilde(&f.path)).small()).truncate().sense(Sense::click()))
                                    .on_hover_text(t(K::ClickReveal))
                                    .clicked()
                                {
                                    reveal_in_finder(&f.path);
                                }
                            });
                        }
                    });
                ui.add_space(5.0);
            }
        });
    }

    // ───────────────────────── Απεγκατάσταση ─────────────────────────
    fn ui_uninstall(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        header(ui, t(K::NavUninstall), t(K::UninstallSub));
        if !self.apps_loaded {
            if self.busy_view(ui) {
                return;
            }
            self.spawn(ctx, t(K::ReadingApps), |c| Msg::Apps(apps::list(c)));
            return;
        }
        let avail = ui.available_height();
        ui.horizontal_top(|ui| {
            // Λίστα εφαρμογών
            ui.vertical(|ui| {
                ui.set_width(330.0);
                ui.add(egui::TextEdit::singleline(&mut self.app_filter).hint_text(t(K::SearchHint)).desired_width(320.0));
                let f = self.app_filter.to_lowercase();
                egui::ScrollArea::vertical().id_salt("apps").max_height(avail - 40.0).auto_shrink([false, false]).show(ui, |ui| {
                    for (i, a) in self.apps.iter().enumerate() {
                        if !f.is_empty() && !a.name.to_lowercase().contains(&f) {
                            continue;
                        }
                        let sel = self.app_sel == Some(i);
                        let r = ui.add(
                            egui::Button::new(RichText::new(format!("{}   ", a.name)))
                                .shortcut_text(RichText::new(human(a.size)).small().color(MUTED))
                                .fill(if sel { ACCENT } else { Color32::TRANSPARENT })
                                .min_size(Vec2::new(310.0, 30.0)),
                        );
                        if r.clicked() && !sel {
                            self.app_sel = Some(i);
                            self.leftovers.clear();
                            let app = a.clone();
                            let tx = self.tx.clone();
                            let ctx2 = ctx.clone();
                            let c = self.counter.clone();
                            std::thread::spawn(move || {
                                let items = apps::leftovers(&app, &c);
                                let _ = tx.send(Msg::Leftovers(app.id.clone(), items));
                                ctx2.request_repaint();
                            });
                        }
                    }
                });
            });
            ui.separator();
            // Λεπτομέρειες
            ui.vertical(|ui| {
                let Some(app) = self.app_sel.and_then(|i| self.apps.get(i)).cloned() else {
                    ui.add_space(60.0);
                    ui.label(RichText::new(t(K::PickApp)).color(MUTED).size(16.0));
                    return;
                };
                ui.label(RichText::new(&app.name).size(22.0).strong());
                ui.label(RichText::new(format!("{}  ·  {}", app.id, tf(K::Version, &[&app.version]))).small().color(MUTED));
                ui.label(RichText::new(app.path.display().to_string()).small().color(MUTED));
                ui.add_space(8.0);
                let left: u64 = self.leftovers.iter().filter(|i| i.selected).map(|i| i.size).sum();
                ui.horizontal(|ui| {
                    ui.label(t(K::AppLabel));
                    size_label(ui, app.size);
                    ui.label(t(K::LeftoversLabel));
                    size_label(ui, left);
                });
                ui.add_space(6.0);
                if self.busy.is_some() {
                    ui.spinner();
                } else if ui
                    .add(|ui: &mut egui::Ui| secondary(ui, &tf(K::UninstallBtn, &[&human(app.size + left)]), DANGER))
                    .clicked()
                {
                    self.confirm = Some(Confirm::Uninstall);
                }
                ui.add_space(8.0);
                ui.label(RichText::new(t(K::RelatedFiles)).strong());
                egui::ScrollArea::vertical().id_salt("left").auto_shrink([false, false]).show(ui, |ui| {
                    if self.leftovers.is_empty() {
                        ui.label(RichText::new(t(K::NotFoundSearching)).small().color(MUTED));
                    }
                    for it in &mut self.leftovers {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut it.selected, "");
                            ui.add(egui::Label::new(RichText::new(tilde(&it.path)).small()).truncate());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(human(it.size)).small().monospace());
                            });
                        });
                    }
                });
            });
        });
    }

    // ───────────────────────── Χάρτης χώρου ─────────────────────────
    fn ui_lens(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        header(ui, t(K::LensTitle), t(K::LensSub));
        let mut go: Option<PathBuf> = None;
        ui.horizontal(|ui| {
            if ui.add_enabled(self.lens_path.parent().is_some(), egui::Button::new(t(K::Up))).clicked() {
                go = self.lens_path.parent().map(|p| p.to_path_buf());
            }
            if ui.button(t(K::Home)).clicked() {
                go = Some(home());
            }
            if ui.button(t(K::Applications)).clicked() {
                go = Some(PathBuf::from("/Applications"));
            }
            if ui.button(t(K::Library)).clicked() {
                go = Some(home().join("Library"));
            }
            if ui.button("↻").on_hover_text(t(K::Refresh)).clicked() {
                self.lens_loaded_for = None;
            }
            ui.label(RichText::new(tilde(&self.lens_path)).monospace().color(ACCENT2));
        });
        if let Some(p) = go {
            self.lens_path = p;
        }
        if self.busy_view(ui) {
            return;
        }
        if self.lens_loaded_for.as_ref() != Some(&self.lens_path) {
            let p = self.lens_path.clone();
            self.lens.clear();
            self.spawn(ctx, t(K::Measuring), move |c| {
                let e = lens::list(&p, c);
                Msg::Lens(p, e)
            });
            return;
        }
        let total: u64 = self.lens.iter().map(|e| e.size).sum();
        ui.label(RichText::new(tf(K::Total, &[&human(total)])).strong());
        ui.separator();
        let max = self.lens.first().map(|e| e.size).unwrap_or(1).max(1);
        let mut enter: Option<PathBuf> = None;
        let mut trash: Option<(PathBuf, u64)> = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for e in self.lens.iter().take(ROW_LIMIT) {
                ui.horizontal(|ui| {
                    let w = 220.0;
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 14.0), Sense::hover());
                    ui.painter().rect_filled(rect, 7.0, Color32::from_rgb(40, 40, 56));
                    let mut fill = rect;
                    fill.set_width((w * e.size as f32 / max as f32).max(4.0));
                    ui.painter().rect_filled(fill, 7.0, if e.is_dir { ACCENT } else { ACCENT2 });
                    ui.add_sized([80.0, 20.0], egui::Label::new(RichText::new(human(e.size)).monospace()));
                    let icon = if e.is_dir { "📁" } else { "📄" };
                    let r = ui.add(egui::Label::new(format!("{} {}", icon, e.name)).truncate().sense(Sense::click()));
                    if r.clicked() && e.is_dir && ext_lower(&e.path) != "app" {
                        enter = Some(e.path.clone());
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if is_safe_target(&e.path) && ui.small_button("🗑").on_hover_text(t(K::MoveToTrash)).clicked() {
                            trash = Some((e.path.clone(), e.size));
                        }
                        if ui.small_button("🔎").on_hover_text(t(K::ShowInFinder)).clicked() {
                            reveal_in_finder(&e.path);
                        }
                        if !e.is_dir && ui.small_button("↗").clicked() {
                            open_path(&e.path);
                        }
                    });
                });
            }
        });
        if let Some(p) = enter {
            self.lens_path = p;
        }
        if let Some((p, s)) = trash {
            self.confirm = Some(Confirm::TrashLens(p, s));
        }
    }

    // ───────────────────────── Συντήρηση ─────────────────────────
    fn ui_maint(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        header(ui, t(K::NavMaint), t(K::MaintSub));
        for (i, task) in maint::tasks().into_iter().enumerate() {
            egui::Frame::new()
                .fill(Color32::from_rgb(30, 30, 42))
                .corner_radius(10.0)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            let lock = if task.admin { "  🔒" } else { "" };
                            ui.label(RichText::new(format!("{}{}", t(task.name), lock)).strong());
                            ui.label(RichText::new(t(task.desc)).small().color(MUTED));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add_enabled(self.busy.is_none(), egui::Button::new(t(K::Run))).clicked() {
                                self.spawn(ctx, t(task.name), move |_| {
                                    let task = &maint::tasks()[i];
                                    Msg::Maint(match maint::run(task) {
                                        Ok(s) | Err(s) => s,
                                    })
                                });
                            }
                        });
                    });
                });
            ui.add_space(5.0);
        }
        ui.add_space(8.0);
        for l in self.maint_log.iter().rev().take(12) {
            ui.label(RichText::new(l).small().monospace());
        }
    }

    // ───────────────────────── Επιβεβαιώσεις ─────────────────────────
    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(c) = &self.confirm else { return };
        let (title, body, button): (String, String, &str) = match c {
            Confirm::CleanJunk => {
                let s: u64 = self.junk.iter().map(|c| c.selected_size()).sum();
                let has_trash = self.junk.iter().any(|c| c.action == junk::Action::EmptyTrash && c.selected_count() > 0);
                (
                    t(K::Clean).to_string(),
                    tf(K::CleanBody, &[&human(s), if has_trash { t(K::TrashEmptyNote) } else { "" }]),
                    t(K::Clean),
                )
            }
            Confirm::TrashLarge => (
                t(K::MoveToTrash).to_string(),
                tf(K::TrashLargeBody, &[&self.large_sel.len().to_string()]),
                t(K::MoveToTrash),
            ),
            Confirm::TrashDupes => (
                t(K::MoveToTrash).to_string(),
                tf(K::TrashDupesBody, &[&self.dupe_sel.len().to_string()]),
                t(K::MoveToTrash),
            ),
            Confirm::Uninstall => {
                let name = self.app_sel.and_then(|i| self.apps.get(i)).map(|a| a.name.clone()).unwrap_or_default();
                (
                    tf(K::UninstallTitle, &[&name]),
                    tf(K::UninstallBody, &[&name]),
                    t(K::Uninstall),
                )
            }
            Confirm::TrashLens(p, s) => (
                t(K::MoveToTrash).to_string(),
                tf(K::TrashOneBody, &[&tilde(p), &human(*s)]),
                t(K::MoveToTrash),
            ),
        };
        let mut decision: Option<bool> = None;
        egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading(RichText::new(title).size(20.0));
            ui.add_space(6.0);
            ui.label(body);
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if secondary(ui, button, DANGER).clicked() {
                    decision = Some(true);
                }
                if ui.button(t(K::Cancel)).clicked() {
                    decision = Some(false);
                }
            });
        });
        match decision {
            Some(true) => {
                let c = self.confirm.take().unwrap();
                self.execute(c, ctx);
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }

    fn execute(&mut self, c: Confirm, ctx: &egui::Context) {
        match c {
            Confirm::CleanJunk => {
                let cats = self.junk.clone();
                self.spawn(ctx, t(K::Clean), move |c| Msg::Cleaned(junk::clean(&cats, c)));
            }
            Confirm::TrashLarge => {
                let paths: Vec<PathBuf> = self.large_sel.iter().cloned().collect();
                let sizes: std::collections::HashMap<PathBuf, u64> =
                    self.large.iter().map(|f| (f.path.clone(), f.size)).collect();
                self.spawn(ctx, t(K::MoveToTrash), move |_| {
                    let (ok, errors) = trash_many(&paths);
                    let bytes = ok.iter().filter_map(|p| sizes.get(p)).sum();
                    Msg::Removed { ok, bytes, errors }
                });
            }
            Confirm::TrashDupes => {
                // Ασφάλεια: σε κάθε ομάδα μένει τουλάχιστον ένα αρχείο.
                let mut paths = Vec::new();
                let mut bytes = 0;
                for g in &self.dupes {
                    let sel: Vec<&FileInfo> = g.files.iter().filter(|f| self.dupe_sel.contains(&f.path)).collect();
                    let take = if sel.len() == g.files.len() { &sel[1..] } else { &sel[..] };
                    for f in take {
                        paths.push(f.path.clone());
                        bytes += f.size;
                    }
                }
                self.spawn(ctx, t(K::MoveToTrash), move |_| {
                    let (ok, errors) = trash_many(&paths);
                    Msg::Removed { ok, bytes, errors }
                });
            }
            Confirm::Uninstall => {
                let Some(app) = self.app_sel.and_then(|i| self.apps.get(i)).cloned() else { return };
                let items = self.leftovers.clone();
                self.spawn(ctx, t(K::Uninstall), move |_| {
                    let (ok, bytes, errors) = apps::uninstall(&app, &items);
                    Msg::Removed { ok, bytes, errors }
                });
            }
            Confirm::TrashLens(p, s) => {
                self.spawn(ctx, t(K::MoveToTrash), move |_| {
                    let (ok, errors) = trash_many(&[p]);
                    let bytes = if ok.is_empty() { 0 } else { s };
                    Msg::Removed { ok, bytes, errors }
                });
            }
        }
    }
}
