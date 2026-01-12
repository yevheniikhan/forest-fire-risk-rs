use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Planet API key
    #[serde(skip_serializing)]
    pub planet_api_key: Option<String>,
    /// NASA FIRMS MAP key
    #[serde(skip_serializing)]
    pub firms_api_key: Option<String>,
    /// Google Drive path for output
    pub google_drive_path: Option<PathBuf>,
    /// Default output directory
    pub output_directory: PathBuf,
    /// Buffer size around fire points (in km)
    pub fire_buffer_km: f64,
    /// Minimum cluster size for fire areas
    pub min_cluster_points: usize,
    /// Cloud cover threshold for Planet imagery (0-100)
    pub max_cloud_cover: u8,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            planet_api_key: None,
            firms_api_key: None,
            google_drive_path: None,
            output_directory: PathBuf::from("./output"),
            fire_buffer_km: 2.0,
            min_cluster_points: 1,
            max_cloud_cover: 20,
        }
    }
}

impl AppConfig {
    /// Check if Planet API is configured
    pub fn has_planet_key(&self) -> bool {
        self.planet_api_key
            .as_ref()
            .map(|k| !k.is_empty())
            .unwrap_or(false)
    }

    /// Check if FIRMS API is configured
    pub fn has_firms_key(&self) -> bool {
        self.firms_api_key
            .as_ref()
            .map(|k| !k.is_empty())
            .unwrap_or(false)
    }

    /// Mask API key for display (show only first/last 4 chars)
    pub fn masked_planet_key(&self) -> String {
        mask_api_key(self.planet_api_key.as_deref())
    }

    /// Mask FIRMS key for display
    pub fn masked_firms_key(&self) -> String {
        mask_api_key(self.firms_api_key.as_deref())
    }
}

/// Mask an API key for display purposes
fn mask_api_key(key: Option<&str>) -> String {
    match key {
        Some(k) if k.len() > 8 => {
            let first = &k[..4];
            let last = &k[k.len() - 4..];
            format!("{}****{}", first, last)
        }
        Some(k) if !k.is_empty() => "****".to_string(),
        _ => "Не встановлено".to_string(),
    }
}

/// Download task configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    /// Unique task ID
    pub id: String,
    /// Output folder path
    pub output_folder: PathBuf,
    /// Bounding box [min_lon, min_lat, max_lon, max_lat]
    pub bbox: [f64; 4],
    /// Start date for imagery
    pub start_date: chrono::NaiveDate,
    /// End date for imagery
    pub end_date: chrono::NaiveDate,
    /// Task description
    pub description: String,
    /// Whether this is a comparison period
    pub is_comparison: bool,
}

/// Download status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Cancelled,
}

impl DownloadStatus {
    pub fn label(&self) -> &str {
        match self {
            DownloadStatus::Pending => "Очікує",
            DownloadStatus::InProgress => "Завантажується",
            DownloadStatus::Completed => "Завершено",
            DownloadStatus::Failed(_) => "Помилка",
            DownloadStatus::Cancelled => "Скасовано",
        }
    }
}

/// Result of a download operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResult {
    pub task_id: String,
    pub status: DownloadStatus,
    pub files_downloaded: Vec<PathBuf>,
    pub total_size_bytes: u64,
    pub duration_seconds: f64,
    pub error_message: Option<String>,
}
