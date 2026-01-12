use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{ComparisonImageryConfig, DownloadMode, FireDetectionPeriod};

/// Session log entry that tracks all download operations
/// Note: API keys are NEVER stored in logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLog {
    /// Unique session ID
    pub session_id: String,
    /// Session start time
    pub started_at: DateTime<Utc>,
    /// Session end time (if completed)
    pub ended_at: Option<DateTime<Utc>>,
    /// Selected oblast/region name
    pub region_name: String,
    /// Download mode used
    pub download_mode: DownloadMode,
    /// Output folder path
    pub output_folder: PathBuf,
    /// Fire detection period (if applicable)
    pub fire_period: Option<FireDetectionPeriod>,
    /// Comparison configuration (if applicable)
    pub comparison_config: Option<ComparisonImageryConfig>,
    /// Custom AOI file name (if uploaded)
    pub aoi_file_name: Option<String>,
    /// List of downloaded files
    pub downloaded_files: Vec<DownloadedFile>,
    /// FIRMS data file path
    pub firms_data_file: Option<PathBuf>,
    /// Shape/boundary file path
    pub boundary_file: Option<PathBuf>,
    /// Total bytes downloaded
    pub total_bytes: u64,
    /// Number of successful downloads
    pub successful_downloads: u32,
    /// Number of failed downloads
    pub failed_downloads: u32,
    /// Error messages (if any)
    pub errors: Vec<LogError>,
    /// Data sources used (without sensitive info)
    pub data_sources: Vec<DataSourceInfo>,
}

impl SessionLog {
    pub fn new(session_id: String, region_name: String, download_mode: DownloadMode) -> Self {
        Self {
            session_id,
            started_at: Utc::now(),
            ended_at: None,
            region_name,
            download_mode,
            output_folder: PathBuf::new(),
            fire_period: None,
            comparison_config: None,
            aoi_file_name: None,
            downloaded_files: Vec::new(),
            firms_data_file: None,
            boundary_file: None,
            total_bytes: 0,
            successful_downloads: 0,
            failed_downloads: 0,
            errors: Vec::new(),
            data_sources: Vec::new(),
        }
    }

    pub fn add_downloaded_file(&mut self, file: DownloadedFile) {
        self.total_bytes += file.size_bytes;
        self.successful_downloads += 1;
        self.downloaded_files.push(file);
    }

    pub fn add_error(&mut self, error: LogError) {
        self.failed_downloads += 1;
        self.errors.push(error);
    }

    pub fn finish(&mut self) {
        self.ended_at = Some(Utc::now());
    }

    /// Generate log file content (safe, no API keys)
    pub fn to_log_string(&self) -> String {
        let mut log = String::new();

        log.push_str("═══════════════════════════════════════════════════════════════\n");
        log.push_str("                    FOREST FIRE RISK RS - DOWNLOAD LOG\n");
        log.push_str("═══════════════════════════════════════════════════════════════\n\n");

        log.push_str(&format!("Session ID: {}\n", self.session_id));
        log.push_str(&format!("Started: {}\n", self.started_at.format("%Y-%m-%d %H:%M:%S UTC")));
        if let Some(ended) = self.ended_at {
            log.push_str(&format!("Ended: {}\n", ended.format("%Y-%m-%d %H:%M:%S UTC")));
            let duration = ended - self.started_at;
            log.push_str(&format!("Duration: {} seconds\n", duration.num_seconds()));
        }

        log.push_str("\n───────────────────────────────────────────────────────────────\n");
        log.push_str("                         CONFIGURATION\n");
        log.push_str("───────────────────────────────────────────────────────────────\n\n");

        log.push_str(&format!("Region: {}\n", self.region_name));
        log.push_str(&format!("Mode: {}\n", self.download_mode.label()));
        log.push_str(&format!("Output Folder: {}\n", self.output_folder.display()));

        if let Some(ref aoi) = self.aoi_file_name {
            log.push_str(&format!("AOI File: {}\n", aoi));
        }

        if let Some(ref period) = self.fire_period {
            log.push_str(&format!("\nFire Detection Period: {}\n", period.format_range()));
        }

        if let Some(ref config) = self.comparison_config {
            log.push_str("\nComparison Imagery Settings:\n");
            log.push_str(&format!(
                "  - Previous months: {} (count: {})\n",
                if config.include_previous_months { "Yes" } else { "No" },
                config.previous_months_count
            ));
            log.push_str(&format!(
                "  - Previous year same months: {} (years: {})\n",
                if config.include_previous_year { "Yes" } else { "No" },
                config.previous_years_count
            ));
        }

        log.push_str("\n───────────────────────────────────────────────────────────────\n");
        log.push_str("                         DATA SOURCES\n");
        log.push_str("───────────────────────────────────────────────────────────────\n\n");

        for source in &self.data_sources {
            log.push_str(&format!("• {}: {}\n", source.name, source.description));
            if let Some(ref url) = source.base_url {
                log.push_str(&format!("  URL: {}\n", url));
            }
        }

        log.push_str("\n───────────────────────────────────────────────────────────────\n");
        log.push_str("                         DOWNLOADED FILES\n");
        log.push_str("───────────────────────────────────────────────────────────────\n\n");

        if let Some(ref firms) = self.firms_data_file {
            log.push_str(&format!("FIRMS Data: {}\n", firms.display()));
        }

        if let Some(ref boundary) = self.boundary_file {
            log.push_str(&format!("Boundary File: {}\n", boundary.display()));
        }

        log.push_str(&format!("\nTotal Files: {}\n", self.downloaded_files.len()));
        log.push_str(&format!("Total Size: {} MB\n", self.total_bytes as f64 / 1_048_576.0));

        for file in &self.downloaded_files {
            log.push_str(&format!(
                "\n  • {}\n    Type: {} | Size: {} KB | Date: {}\n",
                file.file_name,
                file.file_type,
                file.size_bytes / 1024,
                file.downloaded_at.format("%Y-%m-%d %H:%M")
            ));
        }

        log.push_str("\n───────────────────────────────────────────────────────────────\n");
        log.push_str("                           SUMMARY\n");
        log.push_str("───────────────────────────────────────────────────────────────\n\n");

        log.push_str(&format!("Successful downloads: {}\n", self.successful_downloads));
        log.push_str(&format!("Failed downloads: {}\n", self.failed_downloads));

        if !self.errors.is_empty() {
            log.push_str("\nErrors:\n");
            for error in &self.errors {
                log.push_str(&format!(
                    "  [{}] {}: {}\n",
                    error.timestamp.format("%H:%M:%S"),
                    error.operation,
                    error.message
                ));
            }
        }

        log.push_str("\n═══════════════════════════════════════════════════════════════\n");
        log.push_str("                         END OF LOG\n");
        log.push_str("═══════════════════════════════════════════════════════════════\n");

        log
    }

    /// Save log to file
    pub fn save_to_file(&self, path: &PathBuf) -> std::io::Result<()> {
        std::fs::write(path, self.to_log_string())
    }
}

/// Information about a downloaded file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadedFile {
    pub file_name: String,
    pub file_path: PathBuf,
    pub file_type: FileType,
    pub size_bytes: u64,
    pub downloaded_at: DateTime<Utc>,
    pub source: String,
    pub bbox: Option<[f64; 4]>,
    pub date_range: Option<(chrono::NaiveDate, chrono::NaiveDate)>,
}

/// Type of downloaded file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    Satellite,
    FireData,
    Boundary,
    Metadata,
    Other,
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileType::Satellite => write!(f, "Satellite Imagery"),
            FileType::FireData => write!(f, "Fire Data"),
            FileType::Boundary => write!(f, "Boundary"),
            FileType::Metadata => write!(f, "Metadata"),
            FileType::Other => write!(f, "Other"),
        }
    }
}

/// Error log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogError {
    pub timestamp: DateTime<Utc>,
    pub operation: String,
    pub message: String,
    pub recoverable: bool,
}

impl LogError {
    pub fn new(operation: impl Into<String>, message: impl Into<String>, recoverable: bool) -> Self {
        Self {
            timestamp: Utc::now(),
            operation: operation.into(),
            message: message.into(),
            recoverable,
        }
    }
}

/// Information about data source (without sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceInfo {
    pub name: String,
    pub description: String,
    pub base_url: Option<String>,
}

impl DataSourceInfo {
    pub fn planet() -> Self {
        Self {
            name: "Planet Labs".to_string(),
            description: "High-resolution satellite imagery".to_string(),
            base_url: Some("https://api.planet.com".to_string()),
        }
    }

    pub fn firms() -> Self {
        Self {
            name: "NASA FIRMS".to_string(),
            description: "Fire Information for Resource Management System".to_string(),
            base_url: Some("https://firms.modaps.eosdis.nasa.gov".to_string()),
        }
    }

    pub fn geoboundaries() -> Self {
        Self {
            name: "GeoBoundaries".to_string(),
            description: "Open administrative boundaries".to_string(),
            base_url: Some("https://www.geoboundaries.org".to_string()),
        }
    }
}
