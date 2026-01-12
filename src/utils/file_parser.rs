use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::models::{AoiFileFormat, AreaOfInterest};

/// Parse an AOI file from various formats
pub fn parse_aoi_file(path: &Path) -> Result<AreaOfInterest> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .context("File has no extension")?;

    let format = AoiFileFormat::from_extension(extension)
        .context(format!("Unsupported file format: {}", extension))?;

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("aoi")
        .to_string();

    let geojson = match format {
        AoiFileFormat::GeoJson => parse_geojson(path)?,
        AoiFileFormat::Shapefile => parse_shapefile(path)?,
        AoiFileFormat::Kml => parse_kml(path)?,
        AoiFileFormat::Kmz => parse_kmz(path)?,
    };

    let bbox = calculate_geojson_bbox(&geojson);

    Ok(AreaOfInterest {
        source_path: path.to_path_buf(),
        format,
        name,
        geometry: geojson,
        bbox,
    })
}

/// Parse GeoJSON file
fn parse_geojson(path: &Path) -> Result<geojson::GeoJson> {
    let file = File::open(path).context("Failed to open GeoJSON file")?;
    let reader = BufReader::new(file);
    let contents: String = std::io::read_to_string(reader)?;
    let geojson: geojson::GeoJson = contents.parse().context("Failed to parse GeoJSON")?;
    Ok(geojson)
}

/// Parse Shapefile
fn parse_shapefile(path: &Path) -> Result<geojson::GeoJson> {
    let mut reader = shapefile::Reader::from_path(path).context("Failed to open Shapefile")?;

    let mut features = Vec::new();

    for result in reader.iter_shapes_and_records() {
        let (shape, record) = result.context("Failed to read shape")?;

        let geometry = shape_to_geojson_geometry(&shape);
        let properties = record_to_properties(&record);

        features.push(geojson::Feature {
            bbox: None,
            geometry,
            id: None,
            properties: Some(properties),
            foreign_members: None,
        });
    }

    Ok(geojson::GeoJson::FeatureCollection(
        geojson::FeatureCollection {
            bbox: None,
            features,
            foreign_members: None,
        },
    ))
}

/// Convert shapefile shape to GeoJSON geometry
fn shape_to_geojson_geometry(shape: &shapefile::Shape) -> Option<geojson::Geometry> {
    match shape {
        shapefile::Shape::Point(p) => Some(geojson::Geometry::new(geojson::Value::Point(vec![
            p.x, p.y,
        ]))),
        shapefile::Shape::PointM(p) => Some(geojson::Geometry::new(geojson::Value::Point(vec![
            p.x, p.y,
        ]))),
        shapefile::Shape::PointZ(p) => Some(geojson::Geometry::new(geojson::Value::Point(vec![
            p.x, p.y, p.z,
        ]))),
        shapefile::Shape::Polyline(pl) => {
            let coords: Vec<Vec<Vec<f64>>> = pl
                .parts()
                .iter()
                .map(|part| part.iter().map(|p| vec![p.x, p.y]).collect())
                .collect();
            if coords.len() == 1 {
                Some(geojson::Geometry::new(geojson::Value::LineString(
                    coords.into_iter().next().unwrap(),
                )))
            } else {
                Some(geojson::Geometry::new(geojson::Value::MultiLineString(
                    coords,
                )))
            }
        }
        shapefile::Shape::Polygon(poly) => {
            let coords: Vec<Vec<Vec<f64>>> = poly
                .rings()
                .iter()
                .map(|ring| match ring {
                    shapefile::PolygonRing::Outer(pts) | shapefile::PolygonRing::Inner(pts) => {
                        pts.iter().map(|p| vec![p.x, p.y]).collect()
                    }
                })
                .collect();
            Some(geojson::Geometry::new(geojson::Value::Polygon(coords)))
        }
        shapefile::Shape::Multipoint(mp) => {
            let coords: Vec<Vec<f64>> = mp.points().iter().map(|p| vec![p.x, p.y]).collect();
            Some(geojson::Geometry::new(geojson::Value::MultiPoint(coords)))
        }
        shapefile::Shape::NullShape => None,
        _ => None,
    }
}

/// Convert shapefile record to GeoJSON properties
fn record_to_properties(_record: &shapefile::dbase::Record) -> serde_json::Map<String, serde_json::Value> {
    // Return empty properties for now - shapefile API has changed
    // Full property extraction would require matching specific field names
    serde_json::Map::new()
}

/// Parse KML file
fn parse_kml(path: &Path) -> Result<geojson::GeoJson> {
    let file = File::open(path).context("Failed to open KML file")?;
    let reader = BufReader::new(file);

    let kml: kml::Kml = kml::KmlReader::from_reader(reader)
        .read()
        .context("Failed to parse KML")?;

    let features = kml_to_features(kml);

    Ok(geojson::GeoJson::FeatureCollection(
        geojson::FeatureCollection {
            bbox: None,
            features,
            foreign_members: None,
        },
    ))
}

/// Parse KMZ file (KML in ZIP archive)
fn parse_kmz(path: &Path) -> Result<geojson::GeoJson> {
    let file = File::open(path).context("Failed to open KMZ file")?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to read KMZ archive")?;

    // Find the KML file in the archive
    let mut kml_content = String::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.name().ends_with(".kml") {
            file.read_to_string(&mut kml_content)?;
            break;
        }
    }

    if kml_content.is_empty() {
        anyhow::bail!("No KML file found in KMZ archive");
    }

    let kml: kml::Kml = kml::KmlReader::from_string(&kml_content)
        .read()
        .context("Failed to parse KML from KMZ")?;

    let features = kml_to_features(kml);

    Ok(geojson::GeoJson::FeatureCollection(
        geojson::FeatureCollection {
            bbox: None,
            features,
            foreign_members: None,
        },
    ))
}

/// Convert KML to GeoJSON features
fn kml_to_features(kml: kml::Kml) -> Vec<geojson::Feature> {
    let mut features = Vec::new();
    collect_kml_features(&kml, &mut features);
    features
}

fn collect_kml_features(kml: &kml::Kml, features: &mut Vec<geojson::Feature>) {
    match kml {
        kml::Kml::KmlDocument(doc) => {
            for element in &doc.elements {
                collect_kml_features(element, features);
            }
        }
        kml::Kml::Document { elements, .. } => {
            for element in elements {
                collect_kml_features(element, features);
            }
        }
        kml::Kml::Folder { elements, .. } => {
            for element in elements {
                collect_kml_features(element, features);
            }
        }
        kml::Kml::Placemark(placemark) => {
            if let Some(geometry) = &placemark.geometry {
                let geojson_geom = kml_geometry_to_geojson(geometry);
                let mut props = serde_json::Map::new();

                if let Some(name) = &placemark.name {
                    props.insert("name".to_string(), serde_json::Value::String(name.clone()));
                }
                if let Some(desc) = &placemark.description {
                    props.insert(
                        "description".to_string(),
                        serde_json::Value::String(desc.clone()),
                    );
                }

                features.push(geojson::Feature {
                    bbox: None,
                    geometry: geojson_geom,
                    id: None,
                    properties: Some(props),
                    foreign_members: None,
                });
            }
        }
        _ => {}
    }
}

fn kml_geometry_to_geojson(geom: &kml::types::Geometry) -> Option<geojson::Geometry> {
    match geom {
        kml::types::Geometry::Point(p) => {
            Some(geojson::Geometry::new(geojson::Value::Point(vec![
                p.coord.x, p.coord.y,
            ])))
        }
        kml::types::Geometry::LineString(ls) => {
            let coords: Vec<Vec<f64>> = ls.coords.iter().map(|c| vec![c.x, c.y]).collect();
            Some(geojson::Geometry::new(geojson::Value::LineString(coords)))
        }
        kml::types::Geometry::LinearRing(lr) => {
            let coords: Vec<Vec<f64>> = lr.coords.iter().map(|c| vec![c.x, c.y]).collect();
            Some(geojson::Geometry::new(geojson::Value::LineString(coords)))
        }
        kml::types::Geometry::Polygon(poly) => {
            let mut rings = Vec::new();

            // Outer ring
            let outer_coords: Vec<Vec<f64>> =
                poly.outer.coords.iter().map(|c| vec![c.x, c.y]).collect();
            rings.push(outer_coords);

            // Inner rings
            for inner in &poly.inner {
                let inner_coords: Vec<Vec<f64>> =
                    inner.coords.iter().map(|c| vec![c.x, c.y]).collect();
                rings.push(inner_coords);
            }

            Some(geojson::Geometry::new(geojson::Value::Polygon(rings)))
        }
        kml::types::Geometry::MultiGeometry(mg) => {
            let geometries: Vec<geojson::Geometry> = mg
                .geometries
                .iter()
                .filter_map(kml_geometry_to_geojson)
                .collect();
            Some(geojson::Geometry::new(geojson::Value::GeometryCollection(
                geometries,
            )))
        }
        _ => None,
    }
}

/// Calculate bounding box for GeoJSON
fn calculate_geojson_bbox(geojson: &geojson::GeoJson) -> Option<[f64; 4]> {
    let mut min_lon = f64::MAX;
    let mut min_lat = f64::MAX;
    let mut max_lon = f64::MIN;
    let mut max_lat = f64::MIN;

    fn process_coords(coords: &[f64], min_lon: &mut f64, min_lat: &mut f64, max_lon: &mut f64, max_lat: &mut f64) {
        if coords.len() >= 2 {
            *min_lon = min_lon.min(coords[0]);
            *min_lat = min_lat.min(coords[1]);
            *max_lon = max_lon.max(coords[0]);
            *max_lat = max_lat.max(coords[1]);
        }
    }

    fn process_geometry(
        geom: &geojson::Geometry,
        min_lon: &mut f64,
        min_lat: &mut f64,
        max_lon: &mut f64,
        max_lat: &mut f64,
    ) {
        match &geom.value {
            geojson::Value::Point(coords) => {
                process_coords(coords, min_lon, min_lat, max_lon, max_lat);
            }
            geojson::Value::MultiPoint(points) => {
                for p in points {
                    process_coords(p, min_lon, min_lat, max_lon, max_lat);
                }
            }
            geojson::Value::LineString(line) => {
                for p in line {
                    process_coords(p, min_lon, min_lat, max_lon, max_lat);
                }
            }
            geojson::Value::MultiLineString(lines) => {
                for line in lines {
                    for p in line {
                        process_coords(p, min_lon, min_lat, max_lon, max_lat);
                    }
                }
            }
            geojson::Value::Polygon(rings) => {
                for ring in rings {
                    for p in ring {
                        process_coords(p, min_lon, min_lat, max_lon, max_lat);
                    }
                }
            }
            geojson::Value::MultiPolygon(polygons) => {
                for poly in polygons {
                    for ring in poly {
                        for p in ring {
                            process_coords(p, min_lon, min_lat, max_lon, max_lat);
                        }
                    }
                }
            }
            geojson::Value::GeometryCollection(geoms) => {
                for g in geoms {
                    process_geometry(g, min_lon, min_lat, max_lon, max_lat);
                }
            }
        }
    }

    match geojson {
        geojson::GeoJson::Geometry(geom) => {
            process_geometry(geom, &mut min_lon, &mut min_lat, &mut max_lon, &mut max_lat);
        }
        geojson::GeoJson::Feature(feature) => {
            if let Some(geom) = &feature.geometry {
                process_geometry(geom, &mut min_lon, &mut min_lat, &mut max_lon, &mut max_lat);
            }
        }
        geojson::GeoJson::FeatureCollection(fc) => {
            for feature in &fc.features {
                if let Some(geom) = &feature.geometry {
                    process_geometry(geom, &mut min_lon, &mut min_lat, &mut max_lon, &mut max_lat);
                }
            }
        }
    }

    if min_lon < f64::MAX {
        Some([min_lon, min_lat, max_lon, max_lat])
    } else {
        None
    }
}

/// Save GeoJSON to file
pub fn save_geojson(geojson: &geojson::GeoJson, path: &Path) -> Result<()> {
    let json = geojson.to_string();
    std::fs::write(path, json).context("Failed to write GeoJSON file")?;
    Ok(())
}
