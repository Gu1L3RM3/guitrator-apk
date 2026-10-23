//! The Help tab: phone setup, quick start and troubleshooting. Static content only.

use eframe::egui::{self, RichText};

const REPO: &str = "https://github.com/Gu1L3RM3/guitrator-apk";

pub fn show(ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.add_space(8.0);
            section(ui, "Quick start");
            steps(
                ui,
                &[
                    "Install guitrator-apk (see the README for the Flatpak or binary).",
                    "Prepare the phone as described in \"Phone setup\" below.",
                    "Connect the phone with a USB data cable and unlock it.",
                    "Accept the \"Allow USB debugging?\" prompt on the phone. Tick \"Always allow\".",
                    "In the Phone tab, press Refresh (or F5) to reload the phone info and apps. Click an app's name or Details for its version, permissions and more. Press Extract to save its APK.",
                ],
            );

            section(ui, "Phone setup");
            steps(
                ui,
                &[
                    "Enable Developer options: Settings > About phone > tap \"Build number\" 7 times. On Samsung, the path is Settings > About phone > Software information.",
                    "Enable USB debugging: Settings > System > Developer options > USB debugging.",
                    "Xiaomi, Redmi and POCO: also enable \"USB debugging (Security settings)\". It may ask you to sign in to a Mi account.",
                    "Use a data cable. Charge-only cables cannot talk to the phone.",
                    "If the phone asks which USB mode to use, pick \"File transfer\" (MTP).",
                ],
            );
            ui.add_space(4.0);
            ui.weak("This app works over USB only. Wireless debugging is not supported.");

            section(ui, "Troubleshooting");
            egui::Grid::new("troubleshooting")
                .striped(true)
                .spacing([16.0, 8.0])
                .min_col_width(200.0)
                .show(ui, |ui| {
                    row(ui, "Device is unauthorized", "Unlock the phone and accept the USB debugging prompt. If no prompt appears, open Developer options, tap \"Revoke USB debugging authorizations\", then reconnect the cable.");
                    row(ui, "No phone found", "Check the cable (it must carry data), confirm USB debugging is on, and run `adb devices` in a terminal. The phone must be listed.");
                    row(ui, "Device is offline", "Unlock the phone, reconnect the cable, or run `adb kill-server` and try again.");
                    row(ui, "More than one device", "Disconnect the other phones or emulators so only one device remains.");
                    row(ui, "App shows a letter icon", "Normal. Some apps ship only adaptive or vector icons, which are not read yet, so a letter is shown.");
                    row(ui, "Extracted app will not install", "Some apps are split into several APK files. The extract button saves only the base APK, which may not install alone.");
                });

            section(ui, "Download from F-Droid");
            ui.label("The F-Droid tab downloads an open-source app by its package name, for example org.fdroid.fdroid. No phone is needed. Tick \"Also install on the connected phone\" to install it right after the download.");

            section(ui, "Command line");
            ui.label("The same features are available from a terminal. Run `guitrator-apk --help` for every command and example.");
            ui.add_space(4.0);
            ui.code("guitrator-apk list\nguitrator-apk pull -p com.example.app -a /data/app/.../base.apk -d ./apks\nguitrator-apk download -p org.fdroid.fdroid --install");

            section(ui, "About");
            ui.horizontal_wrapped(|ui| {
                ui.label("Source, issues and releases:");
                ui.hyperlink_to(REPO, REPO);
            });
            ui.add_space(12.0);
        });
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(12.0);
    ui.label(RichText::new(title).heading().strong());
    ui.add_space(4.0);
}

fn steps(ui: &mut egui::Ui, items: &[&str]) {
    for (i, text) in items.iter().enumerate() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{}.", i + 1)).strong());
            ui.label(*text);
        });
    }
}

fn row(ui: &mut egui::Ui, problem: &str, fix: &str) {
    ui.label(RichText::new(problem).strong());
    ui.label(fix);
    ui.end_row();
}
