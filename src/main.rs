mod api;
mod models;
mod ui;
mod utils;

use eframe::egui;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() -> eframe::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("forest_fire_risk_rs=info".parse().unwrap()))
        .init();

    tracing::info!("Starting Forest Fire Risk RS");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 800.0])
            .with_min_inner_size([700.0, 600.0])
            .with_title("Forest Fire Risk RS - Система аналізу ризику лісових пожеж"),
        ..Default::default()
    };

    eframe::run_native(
        "Forest Fire Risk RS",
        native_options,
        Box::new(|cc| Ok(Box::new(ui::ForestFireApp::new(cc)))),
    )
}
