//! egui/eframe GUI presentation layer.
//!
//! Three tabs: apps on the connected phone, F-Droid downloads, and help.
//! Slow work (adb queries, APK pulls, icon decoding) runs on background threads.
//! Their results land in shared state that the UI reads on every frame.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, Color32, RichText};

use crate::application::{ExtractApk, ListApps, ReadAppDetails, ReadDeviceInfo};
use crate::domain::{AdbCommandExecutor, AdbError, AppInfo, DeviceInfo};
use crate::infrastructure::{cache_dir, download_apk, load_app_icon, IconPixels};
use crate::presentation::details::{self, DetailAction, DetailState};
use crate::presentation::{format, help};

const ACCENT: Color32 = Color32::from_rgb(74, 222, 128);
const ACCENT_DARK: Color32 = Color32::from_rgb(5, 150, 105);
pub(super) const ERROR: Color32 = Color32::from_rgb(248, 113, 113);
const ICON_PNG: &[u8] = include_bytes!("../../assets/icon.png");
const ICON_SIZE: f32 = 40.0;
const PLACEHOLDER_COLORS: [Color32; 6] = [
    Color32::from_rgb(59, 130, 246),
    Color32::from_rgb(168, 85, 247),
    Color32::from_rgb(236, 72, 153),
    Color32::from_rgb(249, 115, 22),
    Color32::from_rgb(234, 179, 8),
    Color32::from_rgb(20, 184, 166),
];

enum ListState {
    Idle,
    Loading,
    Loaded(Vec<AppInfo>),
    Failed(String),
}

enum Status {
    Ready,
    Working(String),
    Done(String),
    Failed(String),
}

#[derive(PartialEq)]
enum Tab {
    Phone,
    FDroid,
    Help,
}

#[derive(PartialEq, Clone, Copy)]
enum SortBy {
    Name,
    ApkSize,
    Memory,
}

impl SortBy {
    fn label(self) -> &'static str {
        match self {
            SortBy::Name => "Name",
            SortBy::ApkSize => "APK size",
            SortBy::Memory => "Memory",
        }
    }

    /// Largest first for size and memory; apps without a value go last. Ties sort by name.
    fn apply(self, apps: &mut [&AppInfo]) {
        let by_name = |a: &&AppInfo, b: &&AppInfo| a.package_name.cmp(&b.package_name);
        match self {
            SortBy::Name => apps.sort_by(by_name),
            SortBy::ApkSize => {
                apps.sort_by(|a, b| b.apk_size.cmp(&a.apk_size).then_with(|| by_name(a, b)))
            }
            SortBy::Memory => {
                apps.sort_by(|a, b| b.memory_kb.cmp(&a.memory_kb).then_with(|| by_name(a, b)))
            }
        }
    }
}

pub struct GuiApp {
    executor: Arc<dyn AdbCommandExecutor>,
    list: Arc<Mutex<ListState>>,
    device: Arc<Mutex<Option<DeviceInfo>>>,
    /// True while the app list is being reloaded; the old list stays on screen meanwhile.
    refreshing: Arc<AtomicBool>,
    /// Package whose details screen is open.
    selected: Option<String>,
    /// Details read for a package; only shown while that package is still the selected one.
    details: Arc<Mutex<Option<(String, DetailState)>>>,
    /// Icons decoded by the background worker, waiting to be uploaded as textures.
    icon_queue: Arc<Mutex<Vec<(String, IconPixels)>>>,
    icons_running: Arc<AtomicBool>,
    textures: HashMap<String, egui::TextureHandle>,
    status: Arc<Mutex<Status>>,
    tab: Tab,
    search: String,
    hide_system: bool,
    sort: SortBy,
    fdroid_package: String,
    install_after: bool,
    destination: PathBuf,
}

impl GuiApp {
    pub fn new(executor: Arc<dyn AdbCommandExecutor>, cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let app = Self {
            executor,
            list: Arc::new(Mutex::new(ListState::Idle)),
            device: Arc::new(Mutex::new(None)),
            refreshing: Arc::new(AtomicBool::new(false)),
            selected: None,
            details: Arc::new(Mutex::new(None)),
            icon_queue: Arc::new(Mutex::new(Vec::new())),
            icons_running: Arc::new(AtomicBool::new(false)),
            textures: HashMap::new(),
            status: Arc::new(Mutex::new(Status::Ready)),
            tab: Tab::Phone,
            search: String::new(),
            hide_system: true,
            sort: SortBy::Name,
            fdroid_package: String::new(),
            install_after: false,
            destination: dirs_next::download_dir().unwrap_or_else(|| PathBuf::from(".")),
        };
        app.spawn_list();
        app
    }

    /// Reloads the device summary and the app list, then starts loading icons.
    /// A list already on screen stays visible until the new one replaces it.
    fn spawn_list(&self) {
        if self.refreshing.swap(true, Ordering::SeqCst) {
            return;
        }
        let refreshing = Arc::clone(&self.refreshing);
        let executor = Arc::clone(&self.executor);
        let (list, device) = (Arc::clone(&self.list), Arc::clone(&self.device));
        let (icon_queue, icons_running) = (
            Arc::clone(&self.icon_queue),
            Arc::clone(&self.icons_running),
        );
        {
            let mut state = list.lock().unwrap();
            if !matches!(*state, ListState::Loaded(_)) {
                *state = ListState::Loading;
            }
        }
        thread::spawn(move || {
            *device.lock().unwrap() = ReadDeviceInfo::new(executor.as_ref()).execute().ok();
            match ListApps::new(executor.as_ref()).execute_detailed() {
                Ok(apps) => {
                    *list.lock().unwrap() = ListState::Loaded(apps.clone());
                    start_icon_worker(executor, apps, icon_queue, icons_running);
                }
                Err(e) => *list.lock().unwrap() = ListState::Failed(friendly(&e)),
            }
            refreshing.store(false, Ordering::SeqCst);
        });
    }

    /// Reads the details of `package` in the background and opens its screen.
    fn open_details(&mut self, package: String) {
        self.selected = Some(package);
        self.spawn_details();
    }

    fn spawn_details(&self) {
        let Some(package) = self.selected.clone() else {
            return;
        };
        let (executor, details) = (Arc::clone(&self.executor), Arc::clone(&self.details));
        *details.lock().unwrap() = Some((package.clone(), DetailState::Loading));
        thread::spawn(move || {
            let state = match ReadAppDetails::new(executor.as_ref()).execute(&package) {
                Ok(d) => DetailState::Loaded(Box::new(d)),
                Err(e) => DetailState::Failed(friendly(&e)),
            };
            let mut slot = details.lock().unwrap();
            // The user may have opened another app meanwhile; do not overwrite its screen.
            if matches!(&*slot, Some((current, _)) if *current == package) {
                *slot = Some((package, state));
            }
        });
    }

    /// Reloads whatever is on screen: the app list, and the open app's details if any.
    fn refresh(&self) {
        self.spawn_list();
        self.spawn_details();
    }

    /// Runs `job` off the UI thread, reflecting progress and outcome in the status bar.
    fn spawn_job(
        &self,
        working: String,
        job: impl FnOnce() -> Result<String, AdbError> + Send + 'static,
    ) {
        let status = Arc::clone(&self.status);
        *status.lock().unwrap() = Status::Working(working);
        thread::spawn(move || {
            *status.lock().unwrap() = match job() {
                Ok(msg) => Status::Done(msg),
                Err(e) => Status::Failed(friendly(&e)),
            };
        });
    }

    fn spawn_extract(&self, app: AppInfo) {
        let executor = Arc::clone(&self.executor);
        let dest = self.destination.clone();
        self.spawn_job(format!("Extracting {}…", app.package_name), move || {
            ExtractApk::new(executor.as_ref())
                .execute(&app.apk_path, &dest, &app.package_name)
                .map(|p| format!("Saved to {}", p.display()))
        });
    }

    fn spawn_download(&self) {
        let package = self.fdroid_package.trim().to_string();
        let (executor, dest, install) = (
            Arc::clone(&self.executor),
            self.destination.clone(),
            self.install_after,
        );
        self.spawn_job(format!("Downloading {package}…"), move || {
            let saved = download_apk(&package, &dest)?;
            if install {
                executor.run(&["install", "-r", &saved.to_string_lossy()])?;
                return Ok(format!("Downloaded and installed {package}"));
            }
            Ok(format!("Saved to {}", saved.display()))
        });
    }

    fn busy(&self) -> bool {
        matches!(*self.status.lock().unwrap(), Status::Working(_))
    }

    /// Uploads icons that the worker finished since the last frame.
    fn upload_icons(&mut self, ctx: &egui::Context) {
        let ready = std::mem::take(&mut *self.icon_queue.lock().unwrap());
        for (package, pixels) in ready {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [pixels.width, pixels.height],
                &pixels.rgba,
            );
            let texture = ctx.load_texture(&package, image, egui::TextureOptions::LINEAR);
            self.textures.insert(package, texture);
        }
    }

    fn phone_tab(&mut self, ui: &mut egui::Ui) {
        if self.selected.is_some() {
            return self.details_screen(ui);
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search apps…")
                .desired_width(f32::INFINITY),
        );

        if let Some(info) = self.device.lock().unwrap().clone() {
            device_card(ui, &info);
        }

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.hide_system, "Hide system apps");
            ui.add_space(12.0);
            ui.label("Sort by");
            egui::ComboBox::from_id_source("sort")
                .selected_text(self.sort.label())
                .show_ui(ui, |ui| {
                    for sort in [SortBy::Name, SortBy::ApkSize, SortBy::Memory] {
                        ui.selectable_value(&mut self.sort, sort, sort.label());
                    }
                });
        });
        ui.add_space(4.0);

        let busy = self.busy();
        let loading_icons = self.icons_running.load(Ordering::SeqCst);
        let mut clicked: Option<AppInfo> = None;
        let mut opened: Option<String> = None;
        match &*self.list.lock().unwrap() {
            ListState::Idle => {}
            ListState::Loading => centered(ui, |ui| {
                ui.spinner();
                ui.label("Looking for your phone…");
            }),
            ListState::Failed(msg) => centered(ui, |ui| {
                ui.label(RichText::new(msg).color(ERROR));
                ui.weak("Plug in the phone via USB, enable USB debugging and accept the prompt, then press Refresh. The Help tab has details.");
            }),
            ListState::Loaded(apps) => {
                let filter = self.search.to_lowercase();
                let mut shown: Vec<&AppInfo> = apps
                    .iter()
                    .filter(|a| !(self.hide_system && a.is_system()))
                    .filter(|a| a.package_name.to_lowercase().contains(&filter))
                    .collect();
                self.sort.apply(&mut shown);

                let running = shown.iter().filter(|a| a.memory_kb.is_some()).count();
                let ram_kb: u64 = shown.iter().filter_map(|a| a.memory_kb).sum();
                let mut summary = format!(
                    "{} of {} apps · {} running · {} RAM used by apps",
                    shown.len(),
                    apps.len(),
                    running,
                    format::kib(ram_kb)
                );
                if loading_icons {
                    summary.push_str(" · loading icons…");
                }
                ui.weak(summary);

                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for app in shown {
                            let texture = self.textures.get(&app.package_name);
                            row(ui, |ui| {
                                app_icon(ui, &app.package_name, texture);
                                ui.vertical(|ui| {
                                    let name = ui
                                        .add(
                                            egui::Label::new(
                                                RichText::new(&app.package_name).strong(),
                                            )
                                            .sense(egui::Sense::click()),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .on_hover_text("Show details");
                                    if name.clicked() {
                                        opened = Some(app.package_name.clone());
                                    }
                                    ui.horizontal(|ui| {
                                        if let Some(size) = app.apk_size {
                                            chip(ui, format!("APK {}", format::bytes(size)));
                                        }
                                        match app.memory_kb {
                                            Some(kb) => {
                                                chip(ui, format!("RAM {}", format::kib(kb)))
                                            }
                                            None => {
                                                ui.weak(RichText::new("not running").small());
                                            }
                                        }
                                    });
                                    ui.weak(RichText::new(&app.apk_path).small());
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if primary_button(ui, "Extract", !busy).clicked() {
                                            clicked = Some(app.clone());
                                        }
                                        if ui.button("Details").clicked() {
                                            opened = Some(app.package_name.clone());
                                        }
                                    },
                                );
                            });
                        }
                    });
            }
        }
        if let Some(app) = clicked {
            self.spawn_extract(app);
        }
        if let Some(package) = opened {
            self.open_details(package);
        }
    }

    fn details_screen(&mut self, ui: &mut egui::Ui) {
        let Some(package) = self.selected.clone() else {
            return;
        };
        let app = match &*self.list.lock().unwrap() {
            ListState::Loaded(apps) => apps.iter().find(|a| a.package_name == package).cloned(),
            _ => None,
        };
        let Some(app) = app else {
            // The app is gone (uninstalled, or the phone was unplugged): back to the list.
            self.selected = None;
            return;
        };

        let busy = self.busy();
        let action = {
            let details = self.details.lock().unwrap();
            let state = match &*details {
                Some((current, state)) if *current == package => Some(state),
                _ => None,
            };
            details::show(ui, &app, state, self.textures.get(&package), busy)
        };
        match action {
            Some(DetailAction::Back) => self.selected = None,
            Some(DetailAction::Extract) => self.spawn_extract(app),
            Some(DetailAction::Refresh) => self.refresh(),
            None => {}
        }
    }

    fn fdroid_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label("Download an app straight from F-Droid. No phone needed.");
        ui.add_space(4.0);
        let mut submit = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.fdroid_package)
                    .hint_text("Package name, e.g. org.fdroid.fdroid")
                    .desired_width(ui.available_width() - 120.0),
            );
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let ready = !self.busy() && !self.fdroid_package.trim().is_empty();
            submit = (primary_button(ui, "Download", ready).clicked() || enter) && ready;
        });
        ui.checkbox(
            &mut self.install_after,
            "Also install on the connected phone",
        );
        if submit {
            self.spawn_download();
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.upload_icons(ctx);
        if self.tab == Tab::Phone && ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.refresh();
        }

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Guitrator APK").color(ACCENT));
                ui.add_space(16.0);
                ui.selectable_value(&mut self.tab, Tab::Phone, "Phone");
                ui.selectable_value(&mut self.tab, Tab::FDroid, "F-Droid");
                ui.selectable_value(&mut self.tab, Tab::Help, "Help");
                if self.tab == Tab::Phone {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let refreshing = self.refreshing.load(Ordering::SeqCst);
                        let button = primary_button(ui, "Refresh", !refreshing)
                            .on_hover_text("Reload the phone info and the app list (F5)");
                        if button.clicked() {
                            self.refresh();
                        }
                        if refreshing {
                            ui.spinner();
                        }
                    });
                }
            });
            ui.add_space(6.0);
        });

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.weak("Save to:");
                ui.label(self.destination.to_string_lossy().as_ref());
                if ui.button("Change…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(&self.destination)
                        .pick_folder()
                    {
                        self.destination = path;
                    }
                }
            });
            ui.horizontal(|ui| match &*self.status.lock().unwrap() {
                Status::Ready => {
                    ui.weak("Ready");
                }
                Status::Working(msg) => {
                    ui.spinner();
                    ui.label(msg);
                }
                Status::Done(msg) => {
                    ui.label(RichText::new(msg).color(ACCENT));
                }
                Status::Failed(msg) => {
                    ui.label(RichText::new(msg).color(ERROR));
                }
            });
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Phone => self.phone_tab(ui),
            Tab::FDroid => self.fdroid_tab(ui),
            Tab::Help => help::show(ui),
        });

        // Background threads change state without waking the UI.
        // Poll faster while icons stream in, so they show up as soon as they are ready.
        let poll_ms = if self.icons_running.load(Ordering::SeqCst) {
            16
        } else {
            100
        };
        ctx.request_repaint_after(std::time::Duration::from_millis(poll_ms));
    }
}

/// Icon lookups run in parallel: each one is a few short `adb` calls, dominated by latency.
const ICON_WORKERS: usize = 4;

/// Extracts icons in the background, user apps first. Only one batch runs at a time.
fn start_icon_worker(
    executor: Arc<dyn AdbCommandExecutor>,
    mut apps: Vec<AppInfo>,
    queue: Arc<Mutex<Vec<(String, IconPixels)>>>,
    running: Arc<AtomicBool>,
) {
    if running.swap(true, Ordering::SeqCst) {
        return;
    }
    apps.sort_by_key(AppInfo::is_system);
    let apps = Arc::new(apps);
    let next = Arc::new(AtomicUsize::new(0));
    let workers_left = Arc::new(AtomicUsize::new(ICON_WORKERS));
    let cache = Arc::new(cache_dir().join("icons"));

    for _ in 0..ICON_WORKERS {
        let (executor, apps, next) = (Arc::clone(&executor), Arc::clone(&apps), Arc::clone(&next));
        let (queue, running, workers_left, cache) = (
            Arc::clone(&queue),
            Arc::clone(&running),
            Arc::clone(&workers_left),
            Arc::clone(&cache),
        );
        thread::spawn(move || {
            while let Some(app) = apps.get(next.fetch_add(1, Ordering::SeqCst)) {
                if let Some(pixels) = load_app_icon(executor.as_ref(), app, &cache) {
                    queue
                        .lock()
                        .unwrap()
                        .push((app.package_name.clone(), pixels));
                }
            }
            if workers_left.fetch_sub(1, Ordering::SeqCst) == 1 {
                running.store(false, Ordering::SeqCst);
            }
        });
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.selection.bg_fill = ACCENT_DARK;
    v.hyperlink_color = ACCENT;
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
    ] {
        w.rounding = egui::Rounding::same(6.0);
    }
    ctx.set_visuals(v);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 6.0);
    ctx.set_style(style);
}

pub(super) fn primary_button(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).strong().color(Color32::WHITE)).fill(ACCENT_DARK),
    )
}

/// A rounded card spanning the full width.
fn row(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .fill(ui.visuals().faint_bg_color)
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(content);
        });
}

/// Small rounded label for one fact about an app, such as its size or memory use.
pub(super) fn chip(ui: &mut egui::Ui, text: String) {
    egui::Frame::none()
        .fill(ui.visuals().widgets.inactive.bg_fill)
        .rounding(egui::Rounding::same(4.0))
        .inner_margin(egui::Margin::symmetric(6.0, 1.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(ACCENT));
        });
}

/// The app's icon when it has been extracted, otherwise a coloured letter tile.
pub(super) fn app_icon(ui: &mut egui::Ui, package: &str, texture: Option<&egui::TextureHandle>) {
    let size = egui::vec2(ICON_SIZE, ICON_SIZE);
    match texture {
        Some(texture) => {
            ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(
                texture.id(),
                size,
            )));
        }
        None => {
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let color = PLACEHOLDER_COLORS[placeholder_index(package)];
            let painter = ui.painter();
            painter.rect_filled(rect, egui::Rounding::same(8.0), color);
            let letter = package
                .rsplit('.')
                .next()
                .and_then(|s| s.chars().next())
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                letter,
                egui::FontId::proportional(18.0),
                Color32::WHITE,
            );
        }
    }
}

/// Stable colour per package, so the same app always gets the same placeholder.
fn placeholder_index(package: &str) -> usize {
    package
        .bytes()
        .fold(0usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize))
        % PLACEHOLDER_COLORS.len()
}

/// Card with the phone's model, Android version, and RAM and storage use.
fn device_card(ui: &mut egui::Ui, info: &DeviceInfo) {
    row(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(&info.model).strong().size(16.0));
            ui.weak(format!("Android {}", info.android_version));
        });
        ui.add_space(24.0);
        ui.vertical(|ui| {
            usage_bar(ui, "RAM", info.ram_total_kb, info.ram_available_kb);
            usage_bar(ui, "Storage", info.storage_total_kb, info.storage_free_kb);
        });
    });
}

/// `free` is what the phone reports as available; the bar shows what is in use.
fn usage_bar(ui: &mut egui::Ui, label: &str, total: Option<u64>, free: Option<u64>) {
    ui.horizontal(|ui| {
        ui.add_sized([64.0, 18.0], egui::Label::new(label));
        match (total, free) {
            (Some(total), Some(free)) if total > 0 => {
                let used = total - free.min(total);
                let fraction = used as f32 / total as f32;
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(240.0)
                        .text(format!(
                            "{} of {} used",
                            format::kib(used),
                            format::kib(total)
                        )),
                );
            }
            _ => {
                ui.weak("unknown");
            }
        }
    });
}

fn centered(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() / 3.0);
        content(ui);
    });
}

fn friendly(e: &AdbError) -> String {
    match e {
        AdbError::NoDevice => "No phone found.".into(),
        other => other.to_string(),
    }
}

pub fn launch(executor: Arc<dyn AdbCommandExecutor>) -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Guitrator APK")
        .with_inner_size([860.0, 620.0])
        .with_min_inner_size([520.0, 380.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
        viewport = viewport.with_icon(icon);
    }
    eframe::run_native(
        "Guitrator APK",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| Box::new(GuiApp::new(executor, cc))),
    )
}
