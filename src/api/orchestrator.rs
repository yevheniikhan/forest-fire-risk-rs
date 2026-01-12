use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::path::PathBuf;
use uuid::Uuid;

use crate::api::{FirmsClient, GeoBoundariesClient, PlanetClient, FireDataSource, PlanetItemType};
use crate::models::{
    AppConfig, AreaOfInterest, ComparisonImageryConfig, DataSourceInfo, DownloadMode,
    DownloadedFile, FileType, FireDetectionPeriod, LogError, Oblast, SessionLog,
};
use crate::utils::{save_geojson, FolderManager};

/// Orchestrates the download process
pub struct DownloadOrchestrator {
    config: AppConfig,
    firms_client: Option<FirmsClient>,
    planet_client: Option<PlanetClient>,
    geoboundaries_client: GeoBoundariesClient,
    folder_manager: FolderManager,
}

impl DownloadOrchestrator {
    pub fn new(config: AppConfig) -> Self {
        let firms_client = config
            .firms_api_key
            .as_ref()
            .filter(|k| !k.is_empty())
            .map(|k| FirmsClient::new(k.clone()));

        let planet_client = config
            .planet_api_key
            .as_ref()
            .filter(|k| !k.is_empty())
            .map(|k| PlanetClient::new(k.clone()));

        let folder_manager = FolderManager::new(config.output_directory.clone());

        Self {
            config,
            firms_client,
            planet_client,
            geoboundaries_client: GeoBoundariesClient::new(),
            folder_manager,
        }
    }

    /// Execute download for fire areas mode
    pub async fn download_fire_areas(
        &self,
        oblast: &Oblast,
        fire_period: &FireDetectionPeriod,
        comparison_config: &ComparisonImageryConfig,
    ) -> Result<SessionLog> {
        let session_id = Uuid::new_v4().to_string();
        let mut log = SessionLog::new(
            session_id.clone(),
            oblast.display_name(),
            DownloadMode::FireAreasOnly,
        );

        log.fire_period = Some(fire_period.clone());
        log.comparison_config = Some(comparison_config.clone());

        // Add data sources info
        log.data_sources.push(DataSourceInfo::firms());
        log.data_sources.push(DataSourceInfo::geoboundaries());
        if self.planet_client.is_some() {
            log.data_sources.push(DataSourceInfo::planet());
        }

        // Create folder structure
        let session_folder = self.folder_manager.create_session_folder(
            &oblast.name_en,
            &DownloadMode::FireAreasOnly,
            Some(fire_period),
        )?;

        log.output_folder = session_folder.clone();

        let structure = self.folder_manager.create_fire_areas_structure(
            &session_folder,
            comparison_config,
            fire_period,
        )?;

        // Get oblast bbox
        let bbox = oblast
            .geometry
            .as_ref()
            .and_then(|g| GeoBoundariesClient::calculate_bbox(g))
            .context("Failed to calculate oblast bounding box")?;

        // Save oblast boundary
        if let Some(ref geom) = oblast.geometry {
            let boundary_path = structure.data_folder.join("oblast_boundary.geojson");
            let geojson = geojson::GeoJson::Geometry(geom.clone());
            save_geojson(&geojson, &boundary_path)?;
            log.boundary_file = Some(boundary_path);
        }

        // Fetch fire data
        let firms_client = self
            .firms_client
            .as_ref()
            .context("FIRMS API key required for fire areas mode")?;

        tracing::info!(
            "Fetching fire data for {} from {} to {}",
            oblast.name_en,
            fire_period.start_date,
            fire_period.end_date
        );

        let fires = firms_client
            .fetch_fires(
                bbox,
                fire_period.start_date,
                fire_period.end_date,
                FireDataSource::ViirsSNppNrt,
            )
            .await?;

        // Save fire data
        let fire_geojson = FirmsClient::fires_to_geojson(&fires);
        let fire_data_path = structure.data_folder.join("firms_fire_data.geojson");
        save_geojson(&fire_geojson, &fire_data_path)?;
        log.firms_data_file = Some(fire_data_path.clone());

        log.add_downloaded_file(DownloadedFile {
            file_name: "firms_fire_data.geojson".to_string(),
            file_path: fire_data_path,
            file_type: FileType::FireData,
            size_bytes: 0, // TODO: Calculate actual size
            downloaded_at: chrono::Utc::now(),
            source: "NASA FIRMS".to_string(),
            bbox: Some(bbox),
            date_range: Some((fire_period.start_date, fire_period.end_date)),
        });

        // Cluster fires
        let clusters = FirmsClient::cluster_fires(fires, self.config.fire_buffer_km);
        tracing::info!("Found {} fire clusters", clusters.len());

        // Process each cluster
        for cluster in &clusters {
            if cluster.points.len() < self.config.min_cluster_points {
                continue;
            }

            let cluster_bbox = cluster.expanded_bbox(self.config.fire_buffer_km);

            // Create cluster folder
            let cluster_folder =
                self.folder_manager.create_cluster_folder(&structure.fire_period_folder, cluster)?;

            // Download imagery for fire period
            if let Some(ref planet) = self.planet_client {
                match self
                    .download_planet_imagery(
                        planet,
                        cluster_bbox,
                        fire_period.start_date,
                        fire_period.end_date,
                        &cluster_folder,
                        &mut log,
                    )
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        log.add_error(LogError::new(
                            "Planet download",
                            format!("Cluster {}: {}", cluster.id, e),
                            true,
                        ));
                    }
                }
            }

            // Download comparison imagery
            for (period, folder) in &structure.comparison_folders {
                let comparison_cluster_folder = folder.join(format!("cluster_{}", cluster.id));
                std::fs::create_dir_all(&comparison_cluster_folder)?;

                if let Some(ref planet) = self.planet_client {
                    match self
                        .download_planet_imagery(
                            planet,
                            cluster_bbox,
                            period.period.start_date,
                            period.period.end_date,
                            &comparison_cluster_folder,
                            &mut log,
                        )
                        .await
                    {
                        Ok(_) => {}
                        Err(e) => {
                            log.add_error(LogError::new(
                                "Planet comparison download",
                                format!("Cluster {} ({}): {}", cluster.id, period.description, e),
                                true,
                            ));
                        }
                    }
                }
            }
        }

        // Save log file
        log.finish();
        let log_path = structure.logs_folder.join("download_log.txt");
        log.save_to_file(&log_path)?;

        // Also save JSON version
        let log_json_path = structure.logs_folder.join("download_log.json");
        let log_json = serde_json::to_string_pretty(&log)?;
        std::fs::write(log_json_path, log_json)?;

        Ok(log)
    }

    /// Execute download for full oblast mode
    pub async fn download_full_oblast(
        &self,
        oblast: &Oblast,
        years: &[i32],
        months: &[u32],
    ) -> Result<SessionLog> {
        let session_id = Uuid::new_v4().to_string();
        let mut log =
            SessionLog::new(session_id, oblast.display_name(), DownloadMode::FullOblast);

        log.data_sources.push(DataSourceInfo::geoboundaries());
        if self.planet_client.is_some() {
            log.data_sources.push(DataSourceInfo::planet());
        }

        let session_folder =
            self.folder_manager
                .create_session_folder(&oblast.name_en, &DownloadMode::FullOblast, None)?;

        log.output_folder = session_folder.clone();

        let structure = self
            .folder_manager
            .create_full_oblast_structure(&session_folder, years, months)?;

        let bbox = oblast
            .geometry
            .as_ref()
            .and_then(|g| GeoBoundariesClient::calculate_bbox(g))
            .context("Failed to calculate oblast bounding box")?;

        // Save oblast boundary
        if let Some(ref geom) = oblast.geometry {
            let boundary_path = structure.data_folder.join("oblast_boundary.geojson");
            let geojson = geojson::GeoJson::Geometry(geom.clone());
            save_geojson(&geojson, &boundary_path)?;
            log.boundary_file = Some(boundary_path);
        }

        // Download imagery for each year/month
        for (year, month, folder) in &structure.year_month_folders {
            let start_date = NaiveDate::from_ymd_opt(*year, *month, 1)
                .context("Invalid date")?;

            let days_in_month = if *month == 12 {
                NaiveDate::from_ymd_opt(*year + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(*year, *month + 1, 1)
            }
            .map(|d| (d - start_date).num_days() as u32)
            .unwrap_or(30);

            let end_date = NaiveDate::from_ymd_opt(*year, *month, days_in_month)
                .unwrap_or(start_date);

            if let Some(ref planet) = self.planet_client {
                match self
                    .download_planet_imagery(planet, bbox, start_date, end_date, folder, &mut log)
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        log.add_error(LogError::new(
                            "Planet download",
                            format!("{}/{}: {}", year, month, e),
                            true,
                        ));
                    }
                }
            }
        }

        log.finish();
        let log_path = structure.logs_folder.join("download_log.txt");
        log.save_to_file(&log_path)?;

        Ok(log)
    }

    /// Execute download for custom AOI
    pub async fn download_aoi(
        &self,
        oblast: &Oblast,
        aoi: &AreaOfInterest,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<SessionLog> {
        let session_id = Uuid::new_v4().to_string();
        let mut log =
            SessionLog::new(session_id, oblast.display_name(), DownloadMode::AreaOfInterest);

        log.aoi_file_name = Some(aoi.file_name());
        log.data_sources.push(DataSourceInfo::geoboundaries());
        if self.planet_client.is_some() {
            log.data_sources.push(DataSourceInfo::planet());
        }

        let period = FireDetectionPeriod::new(start_date, end_date);
        let session_folder = self.folder_manager.create_session_folder(
            &aoi.name,
            &DownloadMode::AreaOfInterest,
            Some(&period),
        )?;

        log.output_folder = session_folder.clone();

        let structure = self
            .folder_manager
            .create_aoi_structure(&session_folder, start_date, end_date)?;

        // Save AOI boundary
        let aoi_path = structure.data_folder.join("aoi_boundary.geojson");
        save_geojson(&aoi.geometry, &aoi_path)?;
        log.boundary_file = Some(aoi_path);

        let bbox = aoi.bbox.context("AOI has no bounding box")?;

        // Download imagery
        if let Some(ref planet) = self.planet_client {
            match self
                .download_planet_imagery(
                    planet,
                    bbox,
                    start_date,
                    end_date,
                    &structure.imagery_folder,
                    &mut log,
                )
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    log.add_error(LogError::new("Planet download", e.to_string(), true));
                }
            }
        }

        log.finish();
        let log_path = structure.logs_folder.join("download_log.txt");
        log.save_to_file(&log_path)?;

        Ok(log)
    }

    /// Download Planet imagery for a bounding box and date range
    async fn download_planet_imagery(
        &self,
        planet: &PlanetClient,
        bbox: [f64; 4],
        start_date: NaiveDate,
        end_date: NaiveDate,
        output_folder: &PathBuf,
        log: &mut SessionLog,
    ) -> Result<()> {
        let item_types = vec![PlanetItemType::PSScene];

        let items = planet
            .search_imagery(
                bbox,
                start_date,
                end_date,
                &item_types,
                self.config.max_cloud_cover as f64,
            )
            .await?;

        tracing::info!("Found {} Planet items", items.len());

        for item in items.iter().take(10) {
            // Limit to 10 items per request
            if let Some(ref links) = item.links {
                if let Some(ref assets_url) = links.assets {
                    // Get assets
                    let item_type = item
                        .properties
                        .item_type
                        .as_deref()
                        .unwrap_or("PSScene");

                    let assets = planet.get_assets(item_type, &item.id).await?;

                    // Look for visual or analytic asset
                    let asset_key = if assets.assets.contains_key("ortho_visual") {
                        "ortho_visual"
                    } else if assets.assets.contains_key("ortho_analytic_4b") {
                        "ortho_analytic_4b"
                    } else {
                        continue;
                    };

                    if let Some(asset) = assets.assets.get(asset_key) {
                        // Activate if needed
                        if asset.status == "inactive" {
                            if let Some(ref links) = asset.links {
                                if let Some(ref activate_url) = links.activate {
                                    planet.activate_asset(activate_url).await?;

                                    // Wait for activation (simplified)
                                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                }
                            }
                        }

                        // Download if ready
                        if let Some(ref location) = asset.location {
                            let file_name = format!("{}_{}.tif", item.id, asset_key);
                            let file_path = output_folder.join(&file_name);

                            let size = planet.download_asset(location, &file_path).await?;

                            log.add_downloaded_file(DownloadedFile {
                                file_name,
                                file_path,
                                file_type: FileType::Satellite,
                                size_bytes: size,
                                downloaded_at: chrono::Utc::now(),
                                source: "Planet Labs".to_string(),
                                bbox: Some(bbox),
                                date_range: Some((start_date, end_date)),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
