//! The "app details" screen: everything Android reports about one installed app.

use eframe::egui::{self, RichText};

use crate::domain::app_details::AppDetails;
use crate::domain::AppInfo;
use crate::presentation::format;

use super::gui::{app_icon, chip, primary_button, ERROR};

pub enum DetailState {
    Loading,
    Loaded(Box<AppDetails>),
    Failed(String),
}

/// What the user asked for on this screen.
pub enum DetailAction {
    Back,
    Extract,
    Refresh,
}

pub fn show(
    ui: &mut egui::Ui,
    app: &AppInfo,
    state: Option<&DetailState>,
    texture: Option<&egui::TextureHandle>,
    busy: bool,
) -> Option<DetailAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        if ui.button("← Back").clicked() {
            action = Some(DetailAction::Back);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if matches!(state, Some(DetailState::Loading)) {
                ui.spinner();
            } else if ui.button("Refresh").clicked() {
                action = Some(DetailAction::Refresh);
            }
        });
    });
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        app_icon(ui, &app.package_name, texture);
        ui.vertical(|ui| {
            ui.label(RichText::new(&app.package_name).strong().size(18.0));
            ui.horizontal(|ui| {
                if let Some(DetailState::Loaded(d)) = state {
                    if let Some(version) = &d.version_name {
                        chip(ui, format!("v{version}"));
                    }
                    chip(ui, if d.system { "System app" } else { "User app" }.into());
                    if d.stopped {
                        chip(ui, "Stopped".into());
                    }
                    if d.disabled {
                        chip(ui, "Disabled".into());
                    }
                }
                chip(
                    ui,
                    if app.memory_kb.is_some() {
                        "Running"
                    } else {
                        "Not running"
                    }
                    .into(),
                );
            });
        });
    });
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        if primary_button(ui, "Extract APK", !busy).clicked() {
            action = Some(DetailAction::Extract);
        }
        if ui.button("Copy package name").clicked() {
            ui.output_mut(|o| o.copied_text = app.package_name.clone());
        }
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| match state {
            None | Some(DetailState::Loading) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Reading app details…");
                });
            }
            Some(DetailState::Failed(msg)) => {
                ui.label(RichText::new(msg).color(ERROR));
            }
            Some(DetailState::Loaded(d)) => facts(ui, app, d),
        });

    action
}

fn facts(ui: &mut egui::Ui, app: &AppInfo, d: &AppDetails) {
    egui::Grid::new("app_facts")
        .num_columns(2)
        .spacing([20.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            fact(
                ui,
                "Version",
                d.version_name.as_deref().map(|v| match d.version_code {
                    Some(code) => format!("{v} (code {code})"),
                    None => v.to_string(),
                }),
            );
            fact(ui, "Android API", api_levels(d));
            fact(ui, "First installed", d.first_install.clone());
            fact(ui, "Last updated", d.last_update.clone());
            fact(ui, "Installed by", d.installer.clone());
            fact(ui, "APK size", app.apk_size.map(format::bytes));
            fact(ui, "Memory in use", app.memory_kb.map(format::kib));
            fact(ui, "CPU", d.cpu_abi.clone());
            fact(ui, "APK path", Some(app.apk_path.clone()));
            fact(ui, "Data folder", d.data_dir.clone());
        });

    ui.add_space(10.0);
    permissions(ui, "Allowed", &d.granted_permissions);
    permissions(ui, "Denied", &d.denied_permissions);
    permissions(ui, "Requested", &d.requested_permissions);
}

/// One row of the facts table; rows without a value are left out.
fn fact(ui: &mut egui::Ui, label: &str, value: Option<String>) {
    let Some(value) = value else { return };
    ui.weak(label);
    ui.add(egui::Label::new(value).wrap(true));
    ui.end_row();
}

fn api_levels(d: &AppDetails) -> Option<String> {
    match (d.min_sdk, d.target_sdk) {
        (Some(min), Some(target)) => Some(format!("min {min}, target {target}")),
        (None, Some(target)) => Some(format!("target {target}")),
        (Some(min), None) => Some(format!("min {min}")),
        (None, None) => None,
    }
}

fn permissions(ui: &mut egui::Ui, title: &str, list: &[String]) {
    if list.is_empty() {
        return;
    }
    egui::CollapsingHeader::new(format!("{title} permissions ({})", list.len()))
        .id_source(title)
        .show(ui, |ui| {
            for permission in list {
                // "android.permission.CAMERA" reads better as "CAMERA".
                ui.label(
                    RichText::new(permission.rsplit('.').next().unwrap_or(permission)).small(),
                )
                .on_hover_text(permission);
            }
        });
}
