//! egui/eframe GUI presentation layer.
//!
//! Two tabs: apps on the connected phone, and F-Droid downloads.
//! Slow work runs on background threads; their outcome lands in `list` / `status`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, Color32, RichText};

use crate::application::{ExtractApk, ListApps};
use crate::domain::{AdbCommandExecutor, AdbError, AppInfo};
use crate::infrastructure::download_apk;

const ACCENT: Color32 = Color32::from_rgb(74, 222, 128);
const ACCENT_DARK: Color32 = Color32::from_rgb(5, 150, 105);
const ERROR: Color32 = Color32::from_rgb(248, 113, 113);
const ICON_PNG: &[u8] = include_bytes!("../../assets/icon.png");

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
}

pub struct GuiApp {
    executor: Arc<dyn AdbCommandExecutor>,
    list: Arc<Mutex<ListState>>,
    status: Arc<Mutex<Status>>,
    tab: Tab,
    search: String,
    hide_system: bool,
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
            status: Arc::new(Mutex::new(Status::Ready)),
            tab: Tab::Phone,
            search: String::new(),
            hide_system: true,
            fdroid_package: String::new(),
            install_after: false,
            destination: dirs_next::download_dir().unwrap_or_else(|| PathBuf::from(".")),
        };
        app.spawn_list();
        app
    }

    fn spawn_list(&self) {
        let (executor, list) = (Arc::clone(&self.executor), Arc::clone(&self.list));
        *list.lock().unwrap() = ListState::Loading;
        thread::spawn(move || {
            *list.lock().unwrap() = match ListApps::new(executor.as_ref()).execute() {
                Ok(apps) => ListState::Loaded(apps),
                Err(e) => ListState::Failed(friendly(&e)),
            };
        });
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

    fn phone_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search apps…")
                    .desired_width(ui.available_width() - 120.0),
            );
            if primary_button(ui, "Refresh", true).clicked() {
                self.spawn_list();
            }
        });
        ui.checkbox(&mut self.hide_system, "Hide system apps");
        ui.add_space(4.0);

        let busy = self.busy();
        let mut clicked: Option<AppInfo> = None;
        match &*self.list.lock().unwrap() {
            ListState::Idle => {}
            ListState::Loading => centered(ui, |ui| {
                ui.spinner();
                ui.label("Looking for your phone…");
            }),
            ListState::Failed(msg) => centered(ui, |ui| {
                ui.label(RichText::new(msg).color(ERROR));
                ui.weak("Plug in the phone via USB, enable USB debugging and accept the prompt, then press Refresh.");
            }),
            ListState::Loaded(apps) => {
                let filter = self.search.to_lowercase();
                let shown: Vec<&AppInfo> = apps
                    .iter()
                    .filter(|a| !(self.hide_system && is_system(a)))
                    .filter(|a| a.package_name.to_lowercase().contains(&filter))
                    .collect();
                ui.weak(format!("{} of {} apps", shown.len(), apps.len()));
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for app in shown {
                            row(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&app.package_name).strong());
                                    ui.weak(RichText::new(&app.apk_path).small());
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if primary_button(ui, "Extract", !busy).clicked() {
                                            clicked = Some(app.clone());
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
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Guitrator APK").color(ACCENT));
                ui.add_space(16.0);
                ui.selectable_value(&mut self.tab, Tab::Phone, "Phone");
                ui.selectable_value(&mut self.tab, Tab::FDroid, "F-Droid");
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
        });

        // Background threads change state without waking the UI.
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
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

fn primary_button(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
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

fn centered(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() / 3.0);
        content(ui);
    });
}

/// Heuristic: apps preinstalled in read-only partitions.
fn is_system(app: &AppInfo) -> bool {
    ["/system", "/product", "/vendor", "/apex", "/system_ext"]
        .iter()
        .any(|p| app.apk_path.starts_with(p))
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
        .with_inner_size([760.0, 560.0])
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
