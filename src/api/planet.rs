use anyhow::{Context, Result};
use chrono::NaiveDate;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

const PLANET_API_BASE: &str = "https://api.planet.com/data/v1";

/// Planet Labs API client
pub struct PlanetClient {
    client: Client,
    api_key: String,
}

impl PlanetClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    /// Search for available imagery
    pub async fn search_imagery(
        &self,
        bbox: [f64; 4],
        start_date: NaiveDate,
        end_date: NaiveDate,
        item_types: &[PlanetItemType],
        max_cloud_cover: f64,
    ) -> Result<Vec<PlanetItem>> {
        let geometry = serde_json::json!({
            "type": "Polygon",
            "coordinates": [[
                [bbox[0], bbox[1]],
                [bbox[2], bbox[1]],
                [bbox[2], bbox[3]],
                [bbox[0], bbox[3]],
                [bbox[0], bbox[1]]
            ]]
        });

        let filter = serde_json::json!({
            "type": "AndFilter",
            "config": [
                {
                    "type": "GeometryFilter",
                    "field_name": "geometry",
                    "config": geometry
                },
                {
                    "type": "DateRangeFilter",
                    "field_name": "acquired",
                    "config": {
                        "gte": format!("{}T00:00:00Z", start_date),
                        "lte": format!("{}T23:59:59Z", end_date)
                    }
                },
                {
                    "type": "RangeFilter",
                    "field_name": "cloud_cover",
                    "config": {
                        "lte": max_cloud_cover / 100.0
                    }
                }
            ]
        });

        let item_type_strs: Vec<&str> = item_types.iter().map(|t| t.as_str()).collect();

        let search_request = serde_json::json!({
            "item_types": item_type_strs,
            "filter": filter
        });

        tracing::info!(
            "Searching Planet imagery: {} to {}, cloud cover <= {}%",
            start_date,
            end_date,
            max_cloud_cover
        );

        let response = self
            .client
            .post(&format!("{}/quick-search", PLANET_API_BASE))
            .basic_auth(&self.api_key, Some(""))
            .json(&search_request)
            .send()
            .await
            .context("Failed to connect to Planet API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Planet API error {}: {}", status, text);
        }

        let search_result: PlanetSearchResponse = response
            .json()
            .await
            .context("Failed to parse Planet search response")?;

        tracing::info!("Found {} items", search_result.features.len());
        Ok(search_result.features)
    }

    /// Get asset information for an item
    pub async fn get_assets(&self, item_type: &str, item_id: &str) -> Result<PlanetAssets> {
        let url = format!("{}/item-types/{}/items/{}/assets", PLANET_API_BASE, item_type, item_id);

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.api_key, Some(""))
            .send()
            .await
            .context("Failed to get assets")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Planet API error {}: {}", status, text);
        }

        let assets: PlanetAssets = response
            .json()
            .await
            .context("Failed to parse assets response")?;

        Ok(assets)
    }

    /// Activate an asset for download
    pub async fn activate_asset(&self, activation_url: &str) -> Result<()> {
        let response = self
            .client
            .post(activation_url)
            .basic_auth(&self.api_key, Some(""))
            .send()
            .await
            .context("Failed to activate asset")?;

        if !response.status().is_success() && response.status().as_u16() != 204 {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Planet activation error {}: {}", status, text);
        }

        Ok(())
    }

    /// Check asset status
    pub async fn check_asset_status(&self, asset_url: &str) -> Result<AssetStatus> {
        let response = self
            .client
            .get(asset_url)
            .basic_auth(&self.api_key, Some(""))
            .send()
            .await
            .context("Failed to check asset status")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Planet API error {}: {}", status, text);
        }

        let asset: PlanetAsset = response
            .json()
            .await
            .context("Failed to parse asset status")?;

        Ok(AssetStatus {
            status: asset.status,
            location: asset.location,
        })
    }

    /// Download an asset to a file
    pub async fn download_asset(&self, download_url: &str, output_path: &PathBuf) -> Result<u64> {
        tracing::info!("Downloading to: {}", output_path.display());

        let response = self
            .client
            .get(download_url)
            .send()
            .await
            .context("Failed to start download")?;

        if !response.status().is_success() {
            let status = response.status();
            anyhow::bail!("Download failed with status {}", status);
        }

        let bytes = response
            .bytes()
            .await
            .context("Failed to read download bytes")?;

        let size = bytes.len() as u64;

        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = File::create(output_path).await?;
        file.write_all(&bytes).await?;

        tracing::info!("Downloaded {} bytes", size);
        Ok(size)
    }

    /// Validate API key
    pub async fn validate_key(&self) -> Result<bool> {
        let response = self
            .client
            .get(&format!("{}/item-types", PLANET_API_BASE))
            .basic_auth(&self.api_key, Some(""))
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}

/// Planet item types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlanetItemType {
    #[default]
    PSOrthoTile,
    PSScene,
    SkySatCollect,
    SkySatScene,
    REOrthoTile,
    REScene,
    Landsat8L1G,
    Sentinel2L1C,
}

impl PlanetItemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanetItemType::PSOrthoTile => "PSOrthoTile",
            PlanetItemType::PSScene => "PSScene",
            PlanetItemType::SkySatCollect => "SkySatCollect",
            PlanetItemType::SkySatScene => "SkySatScene",
            PlanetItemType::REOrthoTile => "REOrthoTile",
            PlanetItemType::REScene => "REScene",
            PlanetItemType::Landsat8L1G => "Landsat8L1G",
            PlanetItemType::Sentinel2L1C => "Sentinel2L1C",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PlanetItemType::PSOrthoTile => "PlanetScope Ortho Tile",
            PlanetItemType::PSScene => "PlanetScope Scene",
            PlanetItemType::SkySatCollect => "SkySat Collect",
            PlanetItemType::SkySatScene => "SkySat Scene",
            PlanetItemType::REOrthoTile => "RapidEye Ortho Tile",
            PlanetItemType::REScene => "RapidEye Scene",
            PlanetItemType::Landsat8L1G => "Landsat 8 L1G",
            PlanetItemType::Sentinel2L1C => "Sentinel-2 L1C",
        }
    }

    pub fn all() -> Vec<PlanetItemType> {
        vec![
            PlanetItemType::PSOrthoTile,
            PlanetItemType::PSScene,
            PlanetItemType::SkySatCollect,
            PlanetItemType::SkySatScene,
            PlanetItemType::Landsat8L1G,
            PlanetItemType::Sentinel2L1C,
        ]
    }
}

/// Planet search response
#[derive(Debug, Deserialize)]
pub struct PlanetSearchResponse {
    pub features: Vec<PlanetItem>,
    #[serde(rename = "_links")]
    pub links: Option<PlanetLinks>,
}

#[derive(Debug, Deserialize)]
pub struct PlanetLinks {
    #[serde(rename = "_self")]
    pub self_link: Option<String>,
    #[serde(rename = "_next")]
    pub next: Option<String>,
}

/// Planet item
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanetItem {
    pub id: String,
    #[serde(rename = "_links")]
    pub links: Option<PlanetItemLinks>,
    pub properties: PlanetItemProperties,
    pub geometry: Option<geojson::Geometry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanetItemLinks {
    pub assets: Option<String>,
    #[serde(rename = "_self")]
    pub self_link: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanetItemProperties {
    pub acquired: Option<String>,
    pub cloud_cover: Option<f64>,
    pub cloud_percent: Option<f64>,
    pub item_type: Option<String>,
    pub satellite_id: Option<String>,
    pub sun_azimuth: Option<f64>,
    pub sun_elevation: Option<f64>,
    pub pixel_resolution: Option<f64>,
}

/// Planet assets
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanetAssets {
    #[serde(flatten)]
    pub assets: std::collections::HashMap<String, PlanetAsset>,
}

/// Planet asset
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanetAsset {
    pub status: String,
    #[serde(rename = "type")]
    pub asset_type: Option<String>,
    pub location: Option<String>,
    #[serde(rename = "_links")]
    pub links: Option<PlanetAssetLinks>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanetAssetLinks {
    pub activate: Option<String>,
    #[serde(rename = "_self")]
    pub self_link: Option<String>,
}

/// Asset status result
#[derive(Debug)]
pub struct AssetStatus {
    pub status: String,
    pub location: Option<String>,
}

impl AssetStatus {
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn is_activating(&self) -> bool {
        self.status == "activating"
    }

    pub fn is_inactive(&self) -> bool {
        self.status == "inactive"
    }
}
