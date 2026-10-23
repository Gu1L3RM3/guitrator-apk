//! egui/eframe GUI presentation layer.
//!
//! State machine: Idle → Loading → Loaded(apps) | Error(msg)
//! Spawns background threads for list and extract so the UI never blocks.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;

use crate::application::{ExtractApk, ListApps};
use crate::domain::{AdbCommandExecutor, AppInfo};

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

enum LoadState {
    Idle,
    Loading,
    Loaded(Vec<AppInfo>),
    /// Reused for status messages too (errors and success notifications).
    Message(String),
}

impl Default for LoadState {
    fn default() -> Self { LoadState::Idle }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct GuiApp {
    executor: Arc<dyn AdbCommandExecutor>,
    state: Arc<Mutex<LoadState>>,
    search: String,
    destination: PathBuf,
}

impl GuiApp {
    pub fn new(executor: Arc<dyn AdbCommandExecutor>, _cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            executor,
            state: Arc::new(Mutex::new(LoadState::Idle)),
            search: String::new(),
            destination: dirs_next::download_dir().unwrap_or_else(|| PathBuf::from(".")),
        }
    }

    fn spawn_list(&self) {
        let executor = Arc::clone(&self.executor);
        let state = Arc::clone(&self.state);
        *state.lock().unwrap() = LoadState::Loading;
        thread::spawn(move || {
            *state.lock().unwrap() = match ListApps::new(executor.as_ref()).execute() {
                Ok(apps) => LoadState::Loaded(apps),
                Err(e) => LoadState::Message(format!("❌ {e}")),
            };
        });
    }

    fn spawn_extract(&self, app: AppInfo) {
        let executor = Arc::clone(&self.executor);
        let state = Arc::clone(&self.state);
        let dest = self.destination.clone();
        thread::spawn(move || {
            *state.lock().unwrap() = match ExtractApk::new(executor.as_ref())
                .execute(&app.apk_path, &dest, &app.package_name)
            {
                Ok(path) => LoadState::Message(format!("✅ Saved: {}", path.display())),
                Err(e) => LoadState::Message(format!("❌ {e}")),
            };
        });
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top bar
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🤖 Guitrator APK Extractor");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 Refresh").clicked() {
                        self.spawn_list();
                    }
                });
            });
        });

        // Bottom bar
        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("📁 Destination:");
                ui.label(self.destination.to_string_lossy().as_ref());
                if ui.button("Browse…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.destination = path;
                    }
                }
            });
        });

        // Central panel
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(&mut self.search);
            });
            ui.separator();

            // Collect which app (if any) was clicked BEFORE releasing the lock.
            // This avoids holding the MutexGuard across the spawn_extract call.
            let mut clicked_app: Option<AppInfo> = None;

            {
                let state = self.state.lock().unwrap();
                match &*state {
                    LoadState::Idle => {
                        ui.centered_and_justified(|ui| {
                            ui.label("Click 🔄 Refresh to list installed apps.");
                        });
                    }
                    LoadState::Loading => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Loading…");
                        });
                    }
                    LoadState::Message(msg) => {
                        ui.label(msg);
                    }
                    LoadState::Loaded(apps) => {
                        let filter = self.search.to_lowercase();
                        let filtered: Vec<&AppInfo> = apps
                            .iter()
                            .filter(|a| a.package_name.to_lowercase().contains(&filter))
                            .collect();

                        ui.label(format!("{} / {} packages", filtered.len(), apps.len()));
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for app in filtered {
                                ui.horizontal(|ui| {
                                    ui.label(&app.package_name);
                                    if ui.small_button("⬇ Extract").clicked() {
                                        clicked_app = Some(app.clone());
                                    }
                                });
                            }
                        });
                    }
                }
            } // MutexGuard dropped here

            if let Some(app) = clicked_app {
                self.spawn_extract(app);
            }
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

pub fn launch(executor: Arc<dyn AdbCommandExecutor>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Guitrator APK Extractor")
            .with_inner_size([700.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Guitrator APK Extractor",
        options,
        Box::new(move |cc| Ok(Box::new(GuiApp::new(executor, cc)))),
    )
}
