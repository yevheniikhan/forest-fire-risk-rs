use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::models::{get_ukrainian_name, Oblast};

const GEOBOUNDARIES_API_BASE: &str = "https://www.geoboundaries.org/api/current/gbOpen";

/// GeoBoundaries API client
pub struct GeoBoundariesClient {
    client: Client,
}

impl GeoBoundariesClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Fetch all ADM1 (oblast-level) boundaries for Ukraine
    pub async fn fetch_ukraine_oblasts(&self) -> Result<Vec<Oblast>> {
        let url = format!("{}/UKR/ADM1/", GEOBOUNDARIES_API_BASE);

        tracing::info!("Fetching Ukraine oblasts from: {}", url);

        let response: GeoBoundariesApiResponse = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to GeoBoundaries API")?
            .json()
            .await
            .context("Failed to parse GeoBoundaries API response")?;

        // Get the GeoJSON download URL
        let geojson_url = response
            .gj_download_url
            .or(response.simplified_geometry_geojson)
            .context("No GeoJSON URL in response")?;

        tracing::info!("Downloading GeoJSON from: {}", geojson_url);

        // Fetch the actual GeoJSON
        let geojson_text = self
            .client
            .get(&geojson_url)
            .send()
            .await
            .context("Failed to download GeoJSON")?
            .text()
            .await
            .context("Failed to read GeoJSON response")?;

        // Parse GeoJSON
        let geojson: geojson::GeoJson =
            geojson_text.parse().context("Failed to parse GeoJSON")?;

        let oblasts = self.parse_oblasts_from_geojson(geojson)?;

        tracing::info!("Successfully loaded {} oblasts", oblasts.len());
        Ok(oblasts)
    }

    /// Parse oblast features from GeoJSON
    fn parse_oblasts_from_geojson(&self, geojson: geojson::GeoJson) -> Result<Vec<Oblast>> {
        let feature_collection = match geojson {
            geojson::GeoJson::FeatureCollection(fc) => fc,
            _ => anyhow::bail!("Expected FeatureCollection"),
        };

        let mut oblasts = Vec::new();

        for feature in feature_collection.features {
            if let Some(props) = &feature.properties {
                let shape_name = props
                    .get("shapeName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let shape_id = props
                    .get("shapeID")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let shape_iso = props
                    .get("shapeISO")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let ukrainian_name = get_ukrainian_name(&shape_name);

                oblasts.push(Oblast {
                    shape_id,
                    name_en: shape_name,
                    name_ua: ukrainian_name,
                    iso_code: shape_iso,
                    geometry: feature.geometry,
                });
            }
        }

        // Sort by Ukrainian name
        oblasts.sort_by(|a, b| a.name_ua.cmp(&b.name_ua));

        Ok(oblasts)
    }

    /// Get a specific oblast by name or ID
    pub async fn get_oblast(&self, name_or_id: &str) -> Result<Option<Oblast>> {
        let oblasts = self.fetch_ukraine_oblasts().await?;

        Ok(oblasts.into_iter().find(|o| {
            o.shape_id == name_or_id
                || o.name_en.to_lowercase() == name_or_id.to_lowercase()
                || o.name_ua.to_lowercase() == name_or_id.to_lowercase()
        }))
    }

    /// Calculate bounding box for an oblast
    pub fn calculate_bbox(geometry: &geojson::Geometry) -> Option<[f64; 4]> {
        use geojson::Value;

        let mut min_lon = f64::MAX;
        let mut min_lat = f64::MAX;
        let mut max_lon = f64::MIN;
        let mut max_lat = f64::MIN;

        fn process_coords(
            coords: &[Vec<f64>],
            min_lon: &mut f64,
            min_lat: &mut f64,
            max_lon: &mut f64,
            max_lat: &mut f64,
        ) {
            for coord in coords {
                if coord.len() >= 2 {
                    *min_lon = min_lon.min(coord[0]);
                    *min_lat = min_lat.min(coord[1]);
                    *max_lon = max_lon.max(coord[0]);
                    *max_lat = max_lat.max(coord[1]);
                }
            }
        }

        fn process_rings(
            rings: &[Vec<Vec<f64>>],
            min_lon: &mut f64,
            min_lat: &mut f64,
            max_lon: &mut f64,
            max_lat: &mut f64,
        ) {
            for ring in rings {
                process_coords(ring, min_lon, min_lat, max_lon, max_lat);
            }
        }

        match &geometry.value {
            Value::Point(coords) => {
                if coords.len() >= 2 {
                    min_lon = coords[0];
                    max_lon = coords[0];
                    min_lat = coords[1];
                    max_lat = coords[1];
                }
            }
            Value::MultiPoint(points) => {
                process_coords(points, &mut min_lon, &mut min_lat, &mut max_lon, &mut max_lat);
            }
            Value::LineString(coords) => {
                process_coords(coords, &mut min_lon, &mut min_lat, &mut max_lon, &mut max_lat);
            }
            Value::MultiLineString(lines) => {
                for line in lines {
                    process_coords(line, &mut min_lon, &mut min_lat, &mut max_lon, &mut max_lat);
                }
            }
            Value::Polygon(rings) => {
                process_rings(rings, &mut min_lon, &mut min_lat, &mut max_lon, &mut max_lat);
            }
            Value::MultiPolygon(polygons) => {
                for polygon in polygons {
                    process_rings(polygon, &mut min_lon, &mut min_lat, &mut max_lon, &mut max_lat);
                }
            }
            Value::GeometryCollection(geometries) => {
                for geom in geometries {
                    if let Some(bbox) = Self::calculate_bbox(geom) {
                        min_lon = min_lon.min(bbox[0]);
                        min_lat = min_lat.min(bbox[1]);
                        max_lon = max_lon.max(bbox[2]);
                        max_lat = max_lat.max(bbox[3]);
                    }
                }
            }
        }

        if min_lon < f64::MAX && min_lat < f64::MAX {
            Some([min_lon, min_lat, max_lon, max_lat])
        } else {
            None
        }
    }
}

impl Default for GeoBoundariesClient {
    fn default() -> Self {
        Self::new()
    }
}

/// API response from GeoBoundaries
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeoBoundariesApiResponse {
    geo_boundary_id: Option<String>,
    #[serde(rename = "boundaryISO")]
    boundary_iso: Option<String>,
    #[serde(rename = "boundaryYear")]
    boundary_year: Option<String>,
    #[serde(rename = "boundaryType")]
    boundary_type: Option<String>,
    #[serde(rename = "gjDownloadURL")]
    gj_download_url: Option<String>,
    #[serde(rename = "simplifiedGeometryGeoJSON")]
    simplified_geometry_geojson: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network
    async fn test_fetch_oblasts() {
        let client = GeoBoundariesClient::new();
        let oblasts = client.fetch_ukraine_oblasts().await.unwrap();
        assert!(!oblasts.is_empty());
        println!("Fetched {} oblasts", oblasts.len());
        for oblast in &oblasts {
            println!("  - {} ({})", oblast.name_ua, oblast.name_en);
        }
    }
}
