use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::path::{Path, PathBuf};

use crate::models::{
    ComparisonImageryConfig, ComparisonPeriod, DownloadMode, FireCluster, FireDetectionPeriod,
};

/// Creates descriptive folder structure for downloads
pub struct FolderManager {
    base_path: PathBuf,
}

impl FolderManager {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Generate output folder path based on configuration
    /// Format: {region}_{mode}_{date_range}_{timestamp}
    pub fn create_session_folder(
        &self,
        region_name: &str,
        mode: &DownloadMode,
        period: Option<&FireDetectionPeriod>,
    ) -> Result<PathBuf> {
        let sanitized_region = sanitize_folder_name(region_name);
        let mode_suffix = mode.short_label();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");

        let folder_name = if let Some(p) = period {
            format!(
                "{}_{}_{}_to_{}_{}",
                sanitized_region,
                mode_suffix,
                p.start_date.format("%Y%m%d"),
                p.end_date.format("%Y%m%d"),
                timestamp
            )
        } else {
            format!("{}_{}_{}", sanitized_region, mode_suffix, timestamp)
        };

        let folder_path = self.base_path.join(folder_name);
        std::fs::create_dir_all(&folder_path).context("Failed to create session folder")?;

        Ok(folder_path)
    }

    /// Create subfolder structure for fire areas mode
    pub fn create_fire_areas_structure(
        &self,
        session_folder: &Path,
        comparison_config: &ComparisonImageryConfig,
        fire_period: &FireDetectionPeriod,
    ) -> Result<FireAreasStructure> {
        // Main fire period folder
        let fire_folder = session_folder.join(format!(
            "fire_period_{}_to_{}",
            fire_period.start_date.format("%Y%m%d"),
            fire_period.end_date.format("%Y%m%d")
        ));
        std::fs::create_dir_all(&fire_folder)?;

        // Comparison periods
        let comparison_periods = comparison_config.generate_comparison_periods(fire_period);
        let mut comparison_folders = Vec::new();

        for period in &comparison_periods {
            let folder = session_folder.join(format!(
                "comparison_{}_{}_to_{}",
                period.folder_suffix(),
                period.period.start_date.format("%Y%m%d"),
                period.period.end_date.format("%Y%m%d")
            ));
            std::fs::create_dir_all(&folder)?;
            comparison_folders.push((period.clone(), folder));
        }

        // Data folder for FIRMS and boundary files
        let data_folder = session_folder.join("data");
        std::fs::create_dir_all(&data_folder)?;

        // Logs folder
        let logs_folder = session_folder.join("logs");
        std::fs::create_dir_all(&logs_folder)?;

        Ok(FireAreasStructure {
            session_folder: session_folder.to_path_buf(),
            fire_period_folder: fire_folder,
            comparison_folders,
            data_folder,
            logs_folder,
        })
    }

    /// Create subfolder for each fire cluster
    pub fn create_cluster_folder(
        &self,
        parent_folder: &Path,
        cluster: &FireCluster,
    ) -> Result<PathBuf> {
        let folder_name = format!(
            "cluster_{}_lat{:.4}_lon{:.4}_{}_to_{}",
            cluster.id,
            cluster.centroid.1,
            cluster.centroid.0,
            cluster.earliest_date.format("%Y%m%d"),
            cluster.latest_date.format("%Y%m%d")
        );

        let folder = parent_folder.join(folder_name);
        std::fs::create_dir_all(&folder)?;

        Ok(folder)
    }

    /// Create full oblast download structure
    pub fn create_full_oblast_structure(
        &self,
        session_folder: &Path,
        years: &[i32],
        months: &[u32],
    ) -> Result<FullOblastStructure> {
        let mut year_month_folders = Vec::new();

        for year in years {
            for month in months {
                let folder = session_folder.join(format!("{}_{:02}", year, month));
                std::fs::create_dir_all(&folder)?;
                year_month_folders.push((*year, *month, folder));
            }
        }

        let data_folder = session_folder.join("data");
        std::fs::create_dir_all(&data_folder)?;

        let logs_folder = session_folder.join("logs");
        std::fs::create_dir_all(&logs_folder)?;

        Ok(FullOblastStructure {
            session_folder: session_folder.to_path_buf(),
            year_month_folders,
            data_folder,
            logs_folder,
        })
    }

    /// Create AOI download structure
    pub fn create_aoi_structure(
        &self,
        session_folder: &Path,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<AoiStructure> {
        let imagery_folder = session_folder.join(format!(
            "imagery_{}_to_{}",
            start_date.format("%Y%m%d"),
            end_date.format("%Y%m%d")
        ));
        std::fs::create_dir_all(&imagery_folder)?;

        let data_folder = session_folder.join("data");
        std::fs::create_dir_all(&data_folder)?;

        let logs_folder = session_folder.join("logs");
        std::fs::create_dir_all(&logs_folder)?;

        Ok(AoiStructure {
            session_folder: session_folder.to_path_buf(),
            imagery_folder,
            data_folder,
            logs_folder,
        })
    }
}

/// Folder structure for fire areas mode
#[derive(Debug)]
pub struct FireAreasStructure {
    pub session_folder: PathBuf,
    pub fire_period_folder: PathBuf,
    pub comparison_folders: Vec<(ComparisonPeriod, PathBuf)>,
    pub data_folder: PathBuf,
    pub logs_folder: PathBuf,
}

/// Folder structure for full oblast mode
#[derive(Debug)]
pub struct FullOblastStructure {
    pub session_folder: PathBuf,
    pub year_month_folders: Vec<(i32, u32, PathBuf)>,
    pub data_folder: PathBuf,
    pub logs_folder: PathBuf,
}

/// Folder structure for AOI mode
#[derive(Debug)]
pub struct AoiStructure {
    pub session_folder: PathBuf,
    pub imagery_folder: PathBuf,
    pub data_folder: PathBuf,
    pub logs_folder: PathBuf,
}

/// Sanitize folder name (remove invalid characters)
fn sanitize_folder_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            ' ' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

/// Generate descriptive folder name for a download task
pub fn generate_task_folder_name(
    region: &str,
    mode: &DownloadMode,
    start_date: NaiveDate,
    end_date: NaiveDate,
    suffix: Option<&str>,
) -> String {
    let base = format!(
        "{}_{}_{}_to_{}",
        sanitize_folder_name(region),
        mode.short_label(),
        start_date.format("%Y%m%d"),
        end_date.format("%Y%m%d")
    );

    if let Some(s) = suffix {
        format!("{}_{}", base, s)
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_folder_name() {
        assert_eq!(sanitize_folder_name("Київська область"), "Київська_область");
        assert_eq!(sanitize_folder_name("Test/Name"), "Test_Name");
        assert_eq!(sanitize_folder_name("  spaces  "), "spaces");
    }
}
