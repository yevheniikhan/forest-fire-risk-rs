use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Download mode selection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DownloadMode {
    /// Download entire oblast area
    #[default]
    FullOblast,
    /// Download custom area of interest (uploaded file)
    AreaOfInterest,
    /// Download only fire-affected regions
    FireAreasOnly,
}

impl DownloadMode {
    pub fn label(&self) -> &'static str {
        match self {
            DownloadMode::FullOblast => "Вся область - Завантажити всю територію",
            DownloadMode::AreaOfInterest => "Зона інтересу - Завантажити власну ділянку",
            DownloadMode::FireAreasOnly => "Лише пожежі - Завантажити тільки території з пожежами",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            DownloadMode::FullOblast => "full_oblast",
            DownloadMode::AreaOfInterest => "aoi",
            DownloadMode::FireAreasOnly => "fire_areas",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            DownloadMode::FullOblast => {
                "Завантажує супутникові знімки для всієї вибраної області"
            }
            DownloadMode::AreaOfInterest => {
                "Завантажте власний файл з межами (GeoJSON, Shapefile, KML, KMZ)"
            }
            DownloadMode::FireAreasOnly => {
                "Завантажує знімки тільки для територій, де були зафіксовані пожежі за вказаний період"
            }
        }
    }
}

/// Supported file formats for Area of Interest upload
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AoiFileFormat {
    GeoJson,
    Shapefile,
    Kml,
    Kmz,
}

impl AoiFileFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "geojson" | "json" => Some(AoiFileFormat::GeoJson),
            "shp" => Some(AoiFileFormat::Shapefile),
            "kml" => Some(AoiFileFormat::Kml),
            "kmz" => Some(AoiFileFormat::Kmz),
            _ => None,
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            AoiFileFormat::GeoJson => &["geojson", "json"],
            AoiFileFormat::Shapefile => &["shp"],
            AoiFileFormat::Kml => &["kml"],
            AoiFileFormat::Kmz => &["kmz"],
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            AoiFileFormat::GeoJson => "GeoJSON",
            AoiFileFormat::Shapefile => "Shapefile",
            AoiFileFormat::Kml => "KML",
            AoiFileFormat::Kmz => "KMZ",
        }
    }
}

/// Area of Interest data loaded from file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AreaOfInterest {
    /// Original file path
    pub source_path: PathBuf,
    /// File format
    pub format: AoiFileFormat,
    /// Name derived from file
    pub name: String,
    /// GeoJSON geometry (converted from any format)
    pub geometry: geojson::GeoJson,
    /// Bounding box [min_lon, min_lat, max_lon, max_lat]
    pub bbox: Option<[f64; 4]>,
}

impl AreaOfInterest {
    pub fn file_name(&self) -> String {
        self.source_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Get all supported file extensions for AOI upload
pub fn supported_aoi_extensions() -> Vec<&'static str> {
    vec!["geojson", "json", "shp", "kml", "kmz"]
}
