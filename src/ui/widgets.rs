use eframe::egui;

/// Custom styled button
pub fn styled_button(ui: &mut egui::Ui, text: &str, accent: bool) -> egui::Response {
    let button = if accent {
        egui::Button::new(text)
            .fill(egui::Color32::from_rgb(255, 140, 0))
            .min_size(egui::vec2(120.0, 30.0))
    } else {
        egui::Button::new(text).min_size(egui::vec2(120.0, 30.0))
    };

    ui.add(button)
}

/// Section header with optional collapse
pub fn section_header(ui: &mut egui::Ui, number: u32, title: &str) {
    ui.add_space(15.0);
    ui.horizontal(|ui| {
        ui.heading(format!("{}. {}", number, title));
    });
    ui.separator();
}

/// Info box
pub fn info_box(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_premultiplied(100, 149, 237, 30))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("ℹ");
                ui.label(text);
            });
        });
}

/// Warning box
pub fn warning_box(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_premultiplied(255, 165, 0, 40))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("⚠");
                ui.colored_label(egui::Color32::from_rgb(200, 100, 0), text);
            });
        });
}

/// Error box
pub fn error_box(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_premultiplied(255, 0, 0, 30))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("❌");
                ui.colored_label(egui::Color32::RED, text);
            });
        });
}

/// Success box
pub fn success_box(ui: &mut egui::Ui, text: &str) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_premultiplied(0, 128, 0, 30))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("✓");
                ui.colored_label(egui::Color32::from_rgb(0, 128, 0), text);
            });
        });
}

/// File info display
pub fn file_info(ui: &mut egui::Ui, name: &str, size_bytes: u64, format: &str) {
    egui::Frame::none()
        .fill(egui::Color32::from_gray(40))
        .inner_margin(6.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("📄");
                ui.label(name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{} | {:.1} KB", format, size_bytes as f64 / 1024.0));
                });
            });
        });
}

/// Progress indicator with label
pub fn labeled_progress(ui: &mut egui::Ui, label: &str, progress: f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::ProgressBar::new(progress)
                .show_percentage()
                .animate(progress < 1.0),
        );
    });
}

/// Date range display
pub fn date_range_label(ui: &mut egui::Ui, start: &chrono::NaiveDate, end: &chrono::NaiveDate) {
    ui.horizontal(|ui| {
        ui.label("📅");
        ui.label(format!(
            "{} — {}",
            start.format("%d.%m.%Y"),
            end.format("%d.%m.%Y")
        ));
        let days = (*end - *start).num_days();
        ui.colored_label(egui::Color32::GRAY, format!("({} днів)", days));
    });
}

/// Bbox display
pub fn bbox_label(ui: &mut egui::Ui, bbox: &[f64; 4]) {
    ui.horizontal(|ui| {
        ui.label("🗺");
        ui.label(format!(
            "({:.4}°, {:.4}°) — ({:.4}°, {:.4}°)",
            bbox[0], bbox[1], bbox[2], bbox[3]
        ));
    });
}

/// Stats display
pub fn stats_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.strong(value);
        });
    });
}

/// Collapsible section
pub fn collapsible_section<R>(
    ui: &mut egui::Ui,
    id: &str,
    title: &str,
    default_open: bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::CollapsingResponse<R> {
    egui::CollapsingHeader::new(title)
        .id_salt(id)
        .default_open(default_open)
        .show(ui, add_contents)
}

/// Tooltip helper
pub fn with_tooltip(ui: &mut egui::Ui, response: egui::Response, tooltip: &str) -> egui::Response {
    response.on_hover_text(tooltip)
}
