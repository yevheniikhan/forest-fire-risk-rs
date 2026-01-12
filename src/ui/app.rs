use chrono::{Datelike, NaiveDate};
use eframe::egui;

use crate::api::{FireDataSource, GeoBoundariesClient};
use crate::models::{
    AppConfig, AreaOfInterest, ComparisonImageryConfig, DownloadMode, FireDetectionPeriod, Oblast,
};

/// Main application state
pub struct ForestFireApp {
    // Configuration
    config: AppConfig,

    // Oblast selection
    oblasts: Vec<Oblast>,
    selected_oblast_idx: Option<usize>,
    oblasts_loading: bool,
    oblasts_error: Option<String>,

    // Download mode
    download_mode: DownloadMode,

    // Area of Interest (custom upload)
    aoi: Option<AreaOfInterest>,
    aoi_error: Option<String>,

    // Fire detection options
    fire_period: FireDetectionPeriod,
    comparison_config: ComparisonImageryConfig,
    #[allow(dead_code)]
    fire_data_source: FireDataSource,

    // Full oblast mode options
    selected_years: Vec<i32>,
    selected_months: Vec<u32>,

    // API Keys
    planet_key_input: String,
    firms_key_input: String,

    // Output
    output_folder_name: String,
    google_drive_path: String,

    // Status
    status_message: Option<(String, bool)>, // (message, is_error)
    is_downloading: bool,
    download_progress: f32,

    // Async runtime handle
    runtime: tokio::runtime::Runtime,
}

impl Default for ForestFireApp {
    fn default() -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

        let today = chrono::Local::now().date_naive();
        let start = today - chrono::Duration::days(30);

        Self {
            config: AppConfig::default(),
            oblasts: Vec::new(),
            selected_oblast_idx: None,
            oblasts_loading: false,
            oblasts_error: None,
            download_mode: DownloadMode::FireAreasOnly,
            aoi: None,
            aoi_error: None,
            fire_period: FireDetectionPeriod::new(start, today),
            comparison_config: ComparisonImageryConfig::default(),
            fire_data_source: FireDataSource::ViirsSNppNrt,
            selected_years: vec![today.year()],
            selected_months: vec![today.month()],
            planet_key_input: String::new(),
            firms_key_input: String::new(),
            output_folder_name: String::new(),
            google_drive_path: String::new(),
            status_message: None,
            is_downloading: false,
            download_progress: 0.0,
            runtime,
        }
    }
}

impl ForestFireApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();

        // Start loading oblasts
        app.load_oblasts();

        app
    }

    fn load_oblasts(&mut self) {
        self.oblasts_loading = true;
        self.oblasts_error = None;

        let client = GeoBoundariesClient::new();

        match self.runtime.block_on(client.fetch_ukraine_oblasts()) {
            Ok(oblasts) => {
                self.oblasts = oblasts;
                self.oblasts_loading = false;
            }
            Err(e) => {
                self.oblasts_error = Some(format!("Помилка завантаження областей: {}", e));
                self.oblasts_loading = false;
            }
        }
    }

    fn show_oblast_selection(&mut self, ui: &mut egui::Ui) {
        ui.heading("1. Оберіть область:");

        if self.oblasts_loading {
            ui.spinner();
            ui.label("Завантаження областей...");
            return;
        }

        if let Some(ref error) = self.oblasts_error {
            ui.colored_label(egui::Color32::RED, error);
            if ui.button("Спробувати знову").clicked() {
                self.load_oblasts();
            }
            return;
        }

        let selected_name = self
            .selected_oblast_idx
            .and_then(|i| self.oblasts.get(i))
            .map(|o| o.display_name())
            .unwrap_or_else(|| "Оберіть область...".to_string());

        egui::ComboBox::from_id_salt("oblast_select")
            .selected_text(selected_name)
            .width(400.0)
            .show_ui(ui, |ui| {
                for (i, oblast) in self.oblasts.iter().enumerate() {
                    ui.selectable_value(
                        &mut self.selected_oblast_idx,
                        Some(i),
                        oblast.display_name(),
                    );
                }
            });
    }

    fn show_download_mode_selection(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.heading("2. Оберіть режим завантаження:");

        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.download_mode,
                DownloadMode::FullOblast,
                "Вся область",
            );
        });
        ui.label("   Завантажити супутникові знімки для всієї області");

        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.download_mode,
                DownloadMode::AreaOfInterest,
                "Зона інтересу",
            );
        });
        ui.label("   Завантажити власний файл з межами (GeoJSON, SHP, KML, KMZ)");

        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.download_mode,
                DownloadMode::FireAreasOnly,
                "Лише пожежі",
            );
        });
        ui.label("   Завантажити тільки території з пожежами");
    }

    fn show_aoi_upload(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.heading("3. Завантажте файл з межами:");

        ui.horizontal(|ui| {
            if ui.button("📂 Обрати файл").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Spatial Files", &["geojson", "json", "shp", "kml", "kmz"])
                    .pick_file()
                {
                    match crate::utils::parse_aoi_file(&path) {
                        Ok(aoi) => {
                            self.aoi = Some(aoi);
                            self.aoi_error = None;
                        }
                        Err(e) => {
                            self.aoi_error = Some(format!("Помилка: {}", e));
                        }
                    }
                }
            }

            if let Some(ref aoi) = self.aoi {
                ui.label(format!("✓ {}", aoi.file_name()));
                if let Some(bbox) = aoi.bbox {
                    ui.label(format!(
                        "({:.2}°, {:.2}°) - ({:.2}°, {:.2}°)",
                        bbox[0], bbox[1], bbox[2], bbox[3]
                    ));
                }
            } else {
                ui.colored_label(egui::Color32::GRAY, "Файл не завантажено");
            }
        });

        if let Some(ref error) = self.aoi_error {
            ui.colored_label(egui::Color32::RED, error);
        }

        ui.label("Підтримувані формати: GeoJSON, Shapefile (.shp), KML, KMZ");
    }

    fn show_fire_detection_options(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.heading("3. Налаштування виявлення пожеж:");

        // Date range
        ui.horizontal(|ui| {
            ui.label("Період пошуку:");
        });

        ui.horizontal(|ui| {
            ui.label("Початок:");
            let mut start = self.fire_period.start_date;
            show_date_picker(ui, &mut start, "start_date");
            self.fire_period.start_date = start;
        });

        ui.horizontal(|ui| {
            ui.label("Кінець:  ");
            let mut end = self.fire_period.end_date;
            show_date_picker(ui, &mut end, "end_date");
            self.fire_period.end_date = end;
        });

        ui.add_space(5.0);

        // Comparison imagery options
        ui.separator();
        ui.label("Порівняльні знімки:");

        ui.checkbox(
            &mut self.comparison_config.include_previous_months,
            "Включити попередні місяці",
        );

        if self.comparison_config.include_previous_months {
            ui.horizontal(|ui| {
                ui.label("   Кількість місяців:");
                ui.add(egui::Slider::new(
                    &mut self.comparison_config.previous_months_count,
                    1..=6,
                ));
            });
        }

        ui.checkbox(
            &mut self.comparison_config.include_previous_year,
            "Ті ж місяці в попередньому році",
        );

        if self.comparison_config.include_previous_year {
            ui.horizontal(|ui| {
                ui.label("   Кількість років:");
                ui.add(egui::Slider::new(
                    &mut self.comparison_config.previous_years_count,
                    1..=3,
                ));
            });
        }

        // Show generated periods
        let periods = self
            .comparison_config
            .generate_comparison_periods(&self.fire_period);
        if !periods.is_empty() {
            ui.add_space(5.0);
            ui.label("Періоди для порівняння:");
            for period in periods {
                ui.label(format!("   • {} ({})", period.description, period.period.format_range()));
            }
        }
    }

    fn show_full_oblast_options(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.heading("3. Оберіть періоди для завантаження:");

        // Years
        ui.horizontal(|ui| {
            ui.label("Роки:");
            let current_year = chrono::Local::now().year();
            for year in (2020..=current_year).rev() {
                let mut selected = self.selected_years.contains(&year);
                if ui.checkbox(&mut selected, year.to_string()).changed() {
                    if selected {
                        if !self.selected_years.contains(&year) {
                            self.selected_years.push(year);
                        }
                    } else {
                        self.selected_years.retain(|&y| y != year);
                    }
                }
            }
        });

        // Months
        ui.label("Місяці:");
        let months = [
            (1, "Січень"),
            (2, "Лютий"),
            (3, "Березень"),
            (4, "Квітень"),
            (5, "Травень"),
            (6, "Червень"),
            (7, "Липень"),
            (8, "Серпень"),
            (9, "Вересень"),
            (10, "Жовтень"),
            (11, "Листопад"),
            (12, "Грудень"),
        ];

        ui.horizontal_wrapped(|ui| {
            for (num, name) in months {
                let mut selected = self.selected_months.contains(&num);
                if ui.checkbox(&mut selected, name).changed() {
                    if selected {
                        if !self.selected_months.contains(&num) {
                            self.selected_months.push(num);
                        }
                    } else {
                        self.selected_months.retain(|&m| m != num);
                    }
                }
            }
        });
    }

    fn show_api_keys(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.heading("4. API ключі:");

        ui.horizontal(|ui| {
            ui.label("Planet:  ");
            ui.add(
                egui::TextEdit::singleline(&mut self.planet_key_input)
                    .password(true)
                    .hint_text("Planet API Key")
                    .desired_width(300.0),
            );
        });

        ui.horizontal(|ui| {
            ui.label("FIRMS:  ");
            ui.add(
                egui::TextEdit::singleline(&mut self.firms_key_input)
                    .password(true)
                    .hint_text("NASA FIRMS MAP Key (обов'язково для режиму пожеж)")
                    .desired_width(300.0),
            );
        });

        if self.download_mode == DownloadMode::FireAreasOnly && self.firms_key_input.is_empty() {
            ui.colored_label(
                egui::Color32::YELLOW,
                "⚠ FIRMS ключ обов'язковий для режиму пожеж",
            );
        }
    }

    fn show_output_options(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.heading("5. Вихідна папка:");

        let default_name = self.generate_default_folder_name();

        ui.horizontal(|ui| {
            ui.label("Назва:");
            ui.add(
                egui::TextEdit::singleline(&mut self.output_folder_name)
                    .hint_text(default_name)
                    .desired_width(300.0),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Google Drive:");
            ui.add(
                egui::TextEdit::singleline(&mut self.google_drive_path)
                    .hint_text("Шлях до Google Drive (опційно)")
                    .desired_width(300.0),
            );
            if ui.button("📂").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.google_drive_path = path.display().to_string();
                }
            }
        });
    }

    fn generate_default_folder_name(&self) -> String {
        let region = self
            .selected_oblast_idx
            .and_then(|i| self.oblasts.get(i))
            .map(|o| o.name_en.replace(' ', "_"))
            .unwrap_or_else(|| "Ukraine".to_string());

        let mode = self.download_mode.short_label();

        format!("{}_{}", region, mode)
    }

    fn show_start_button(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        ui.separator();

        // Validation
        let mut can_start = true;
        let mut validation_errors = Vec::new();

        if self.selected_oblast_idx.is_none() {
            can_start = false;
            validation_errors.push("Оберіть область");
        }

        if self.download_mode == DownloadMode::AreaOfInterest && self.aoi.is_none() {
            can_start = false;
            validation_errors.push("Завантажте файл з межами");
        }

        if self.download_mode == DownloadMode::FireAreasOnly && self.firms_key_input.is_empty() {
            can_start = false;
            validation_errors.push("Введіть FIRMS API ключ");
        }

        if self.download_mode == DownloadMode::FullOblast {
            if self.selected_years.is_empty() {
                can_start = false;
                validation_errors.push("Оберіть хоча б один рік");
            }
            if self.selected_months.is_empty() {
                can_start = false;
                validation_errors.push("Оберіть хоча б один місяць");
            }
        }

        // Show errors
        for error in &validation_errors {
            ui.colored_label(egui::Color32::RED, format!("⚠ {}", error));
        }

        // Progress
        if self.is_downloading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Завантаження...");
            });
            ui.add(egui::ProgressBar::new(self.download_progress).show_percentage());
        }

        // Status message
        if let Some((ref msg, is_error)) = self.status_message {
            let color = if is_error {
                egui::Color32::RED
            } else {
                egui::Color32::GREEN
            };
            ui.colored_label(color, msg);
        }

        // Start button
        ui.horizontal(|ui| {
            let button = egui::Button::new("🚀 Почати завантаження")
                .min_size(egui::vec2(200.0, 40.0));

            if ui
                .add_enabled(can_start && !self.is_downloading, button)
                .clicked()
            {
                self.start_download();
            }

            if self.is_downloading {
                if ui.button("⏹ Скасувати").clicked() {
                    self.is_downloading = false;
                    self.status_message = Some(("Завантаження скасовано".to_string(), true));
                }
            }
        });
    }

    fn start_download(&mut self) {
        self.is_downloading = true;
        self.download_progress = 0.0;
        self.status_message = None;

        // Update config with API keys
        self.config.planet_api_key = Some(self.planet_key_input.clone());
        self.config.firms_api_key = Some(self.firms_key_input.clone());

        // TODO: Implement actual download logic in separate async task
        self.status_message = Some((
            "Завантаження налаштовано. Функціонал в розробці.".to_string(),
            false,
        ));
        self.is_downloading = false;
    }
}

/// Date picker as a free function
fn show_date_picker(ui: &mut egui::Ui, date: &mut NaiveDate, id: &str) {
    let mut day = date.day();
    let mut month = date.month();
    let mut year = date.year();

    ui.horizontal(|ui| {
        // Day
        egui::ComboBox::from_id_salt(format!("{}_day", id))
            .selected_text(format!("{:02}", day))
            .width(50.0)
            .show_ui(ui, |ui| {
                for d in 1..=31 {
                    ui.selectable_value(&mut day, d, format!("{:02}", d));
                }
            });

        ui.label(".");

        // Month
        let month_names = [
            "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12",
        ];
        egui::ComboBox::from_id_salt(format!("{}_month", id))
            .selected_text(month_names[(month - 1) as usize])
            .width(50.0)
            .show_ui(ui, |ui| {
                for (i, m) in month_names.iter().enumerate() {
                    ui.selectable_value(&mut month, (i + 1) as u32, *m);
                }
            });

        ui.label(".");

        // Year
        let current_year = chrono::Local::now().year();
        egui::ComboBox::from_id_salt(format!("{}_year", id))
            .selected_text(year.to_string())
            .width(70.0)
            .show_ui(ui, |ui| {
                for y in (2020..=current_year).rev() {
                    ui.selectable_value(&mut year, y, y.to_string());
                }
            });
    });

    // Update date if changed
    if let Some(new_date) = NaiveDate::from_ymd_opt(year, month, day) {
        *date = new_date;
    }
}

impl eframe::App for ForestFireApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("🔥 Forest Fire Risk RS");
                ui.label("Система аналізу ризику лісових пожеж");
                ui.separator();

                // 1. Oblast selection
                self.show_oblast_selection(ui);

                // 2. Download mode
                self.show_download_mode_selection(ui);

                // 3. Mode-specific options
                match self.download_mode {
                    DownloadMode::FullOblast => {
                        self.show_full_oblast_options(ui);
                    }
                    DownloadMode::AreaOfInterest => {
                        self.show_aoi_upload(ui);
                    }
                    DownloadMode::FireAreasOnly => {
                        self.show_fire_detection_options(ui);
                    }
                }

                // 4. API Keys
                self.show_api_keys(ui);

                // 5. Output options
                self.show_output_options(ui);

                // Start button
                self.show_start_button(ui);
            });
        });
    }
}
