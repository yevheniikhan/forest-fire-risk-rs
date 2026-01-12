use anyhow::{Context, Result};
use chrono::NaiveDate;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::models::{FireCluster, FirePoint};

const FIRMS_API_BASE: &str = "https://firms.modaps.eosdis.nasa.gov/api/area/csv";

/// NASA FIRMS API client
pub struct FirmsClient {
    client: Client,
    api_key: String,
}

impl FirmsClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    /// Fetch fire data for a bounding box and date range
    /// Source can be: VIIRS_SNPP_NRT, VIIRS_NOAA20_NRT, MODIS_NRT, etc.
    pub async fn fetch_fires(
        &self,
        bbox: [f64; 4],
        start_date: NaiveDate,
        end_date: NaiveDate,
        source: FireDataSource,
    ) -> Result<Vec<FirePoint>> {
        let days = (end_date - start_date).num_days();
        if days > 365 {
            anyhow::bail!("Date range cannot exceed 365 days");
        }

        // Format: west,south,east,north
        let area = format!(
            "{},{},{},{}",
            bbox[0], // min_lon (west)
            bbox[1], // min_lat (south)
            bbox[2], // max_lon (east)
            bbox[3]  // max_lat (north)
        );

        let url = format!(
            "{}/{}/{}/{}/{}",
            FIRMS_API_BASE,
            self.api_key,
            source.as_str(),
            area,
            days.max(1)
        );

        tracing::info!("Fetching FIRMS data: {} to {}", start_date, end_date);
        tracing::debug!("FIRMS URL (key masked): {}/***/{}/{}/{}",
            FIRMS_API_BASE, source.as_str(), area, days);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to FIRMS API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("FIRMS API error {}: {}", status, text);
        }

        let csv_text = response
            .text()
            .await
            .context("Failed to read FIRMS response")?;

        let fires = self.parse_csv(&csv_text, start_date, end_date)?;

        tracing::info!("Found {} fire points", fires.len());
        Ok(fires)
    }

    /// Parse CSV response from FIRMS
    fn parse_csv(
        &self,
        csv_text: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<FirePoint>> {
        let mut fires = Vec::new();
        let mut lines = csv_text.lines();

        // Parse header to get column indices
        let header = lines.next().context("Empty CSV response")?;
        let columns: Vec<&str> = header.split(',').collect();

        let lat_idx = columns.iter().position(|&c| c == "latitude");
        let lon_idx = columns.iter().position(|&c| c == "longitude");
        let brightness_idx = columns.iter().position(|&c| c == "bright_ti4" || c == "brightness");
        let acq_date_idx = columns.iter().position(|&c| c == "acq_date");
        let acq_time_idx = columns.iter().position(|&c| c == "acq_time");
        let satellite_idx = columns.iter().position(|&c| c == "satellite");
        let frp_idx = columns.iter().position(|&c| c == "frp");
        let confidence_idx = columns.iter().position(|&c| c == "confidence");
        let daynight_idx = columns.iter().position(|&c| c == "daynight");

        let (lat_idx, lon_idx) = match (lat_idx, lon_idx) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => anyhow::bail!("CSV missing required latitude/longitude columns"),
        };

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() <= lat_idx.max(lon_idx) {
                continue;
            }

            let latitude: f64 = match fields.get(lat_idx).and_then(|s| s.parse().ok()) {
                Some(v) => v,
                None => continue,
            };

            let longitude: f64 = match fields.get(lon_idx).and_then(|s| s.parse().ok()) {
                Some(v) => v,
                None => continue,
            };

            let brightness = brightness_idx
                .and_then(|i| fields.get(i))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let acq_date = acq_date_idx
                .and_then(|i| fields.get(i))
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                .unwrap_or(start_date);

            // Filter by date range
            if acq_date < start_date || acq_date > end_date {
                continue;
            }

            let acq_time = acq_time_idx
                .and_then(|i| fields.get(i))
                .map(|s| s.to_string())
                .unwrap_or_default();

            let satellite = satellite_idx
                .and_then(|i| fields.get(i))
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let frp = frp_idx
                .and_then(|i| fields.get(i))
                .and_then(|s| s.parse().ok());

            let confidence = confidence_idx
                .and_then(|i| fields.get(i))
                .map(|s| s.to_string())
                .unwrap_or_default();

            let daynight = daynight_idx
                .and_then(|i| fields.get(i))
                .map(|s| s.to_string());

            fires.push(FirePoint {
                latitude,
                longitude,
                brightness,
                acq_date,
                acq_time,
                satellite,
                frp,
                confidence,
                daynight,
            });
        }

        Ok(fires)
    }

    /// Cluster fire points by proximity
    pub fn cluster_fires(fires: Vec<FirePoint>, distance_km: f64) -> Vec<FireCluster> {
        if fires.is_empty() {
            return Vec::new();
        }

        let mut clusters: Vec<FireCluster> = Vec::new();
        let mut assigned: Vec<bool> = vec![false; fires.len()];

        for i in 0..fires.len() {
            if assigned[i] {
                continue;
            }

            let mut cluster_points = vec![fires[i].clone()];
            assigned[i] = true;

            // Find all points within distance
            for j in (i + 1)..fires.len() {
                if assigned[j] {
                    continue;
                }

                let dist = haversine_distance(
                    fires[i].latitude,
                    fires[i].longitude,
                    fires[j].latitude,
                    fires[j].longitude,
                );

                if dist <= distance_km {
                    cluster_points.push(fires[j].clone());
                    assigned[j] = true;
                }
            }

            if let Some(cluster) =
                FireCluster::from_points(format!("cluster_{}", clusters.len() + 1), cluster_points)
            {
                clusters.push(cluster);
            }
        }

        clusters
    }

    /// Export fire data to GeoJSON
    pub fn fires_to_geojson(fires: &[FirePoint]) -> geojson::GeoJson {
        let features: Vec<geojson::Feature> = fires
            .iter()
            .map(|fire| {
                let point = geojson::Geometry::new(geojson::Value::Point(vec![
                    fire.longitude,
                    fire.latitude,
                ]));

                let mut properties = serde_json::Map::new();
                properties.insert(
                    "acq_date".to_string(),
                    serde_json::Value::String(fire.acq_date.to_string()),
                );
                properties.insert(
                    "acq_time".to_string(),
                    serde_json::Value::String(fire.acq_time.clone()),
                );
                properties.insert(
                    "brightness".to_string(),
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(fire.brightness).unwrap_or(0.into()),
                    ),
                );
                properties.insert(
                    "satellite".to_string(),
                    serde_json::Value::String(fire.satellite.clone()),
                );
                properties.insert(
                    "confidence".to_string(),
                    serde_json::Value::String(fire.confidence.clone()),
                );
                if let Some(frp) = fire.frp {
                    properties.insert(
                        "frp".to_string(),
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(frp).unwrap_or(0.into()),
                        ),
                    );
                }

                geojson::Feature {
                    bbox: None,
                    geometry: Some(point),
                    id: None,
                    properties: Some(properties),
                    foreign_members: None,
                }
            })
            .collect();

        geojson::GeoJson::FeatureCollection(geojson::FeatureCollection {
            bbox: None,
            features,
            foreign_members: None,
        })
    }
}

/// Calculate distance between two points using Haversine formula
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().asin();

    EARTH_RADIUS_KM * c
}

/// Available fire data sources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FireDataSource {
    /// VIIRS from Suomi NPP - Near Real Time
    #[default]
    ViirsSNppNrt,
    /// VIIRS from NOAA-20 - Near Real Time
    ViirsNoaa20Nrt,
    /// MODIS - Near Real Time
    ModisNrt,
    /// VIIRS from Suomi NPP - Standard
    ViirsSNppSp,
    /// MODIS - Standard
    ModisSp,
}

impl FireDataSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            FireDataSource::ViirsSNppNrt => "VIIRS_SNPP_NRT",
            FireDataSource::ViirsNoaa20Nrt => "VIIRS_NOAA20_NRT",
            FireDataSource::ModisNrt => "MODIS_NRT",
            FireDataSource::ViirsSNppSp => "VIIRS_SNPP_SP",
            FireDataSource::ModisSp => "MODIS_SP",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            FireDataSource::ViirsSNppNrt => "VIIRS S-NPP (NRT)",
            FireDataSource::ViirsNoaa20Nrt => "VIIRS NOAA-20 (NRT)",
            FireDataSource::ModisNrt => "MODIS (NRT)",
            FireDataSource::ViirsSNppSp => "VIIRS S-NPP (Standard)",
            FireDataSource::ModisSp => "MODIS (Standard)",
        }
    }

    pub fn all() -> Vec<FireDataSource> {
        vec![
            FireDataSource::ViirsSNppNrt,
            FireDataSource::ViirsNoaa20Nrt,
            FireDataSource::ModisNrt,
            FireDataSource::ViirsSNppSp,
            FireDataSource::ModisSp,
        ]
    }
}
