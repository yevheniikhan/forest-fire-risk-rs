use serde::{Deserialize, Serialize};

/// Represents a Ukrainian oblast (administrative region)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Oblast {
    /// Internal identifier from GeoBoundaries
    pub shape_id: String,
    /// Name in Ukrainian
    pub name_ua: String,
    /// Name in English
    pub name_en: String,
    /// ISO 3166-2 code (e.g., UA-32 for Kyiv Oblast)
    pub iso_code: Option<String>,
    /// GeoJSON geometry
    pub geometry: Option<geojson::Geometry>,
}

impl Oblast {
    pub fn display_name(&self) -> String {
        format!("{} ({})", self.name_ua, self.name_en)
    }
}

/// Response from GeoBoundaries API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoBoundariesResponse {
    pub geo_boundary_id: Option<String>,
    #[serde(rename = "boundaryISO")]
    pub boundary_iso: Option<String>,
    #[serde(rename = "boundaryYear")]
    pub boundary_year: Option<String>,
    #[serde(rename = "boundaryType")]
    pub boundary_type: Option<String>,
    #[serde(rename = "boundarySource-1")]
    pub boundary_source_1: Option<String>,
    #[serde(rename = "gjDownloadURL")]
    pub geojson_url: Option<String>,
    #[serde(rename = "simplifiedGeometryGeoJSON")]
    pub simplified_geojson_url: Option<String>,
}

/// GeoJSON Feature for oblast boundary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OblastFeature {
    #[serde(rename = "type")]
    pub feature_type: String,
    pub properties: OblastProperties,
    pub geometry: geojson::Geometry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OblastProperties {
    #[serde(rename = "shapeID")]
    pub shape_id: Option<String>,
    #[serde(rename = "shapeName")]
    pub shape_name: Option<String>,
    #[serde(rename = "shapeISO")]
    pub shape_iso: Option<String>,
    #[serde(rename = "shapeGroup")]
    pub shape_group: Option<String>,
    #[serde(rename = "shapeType")]
    pub shape_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OblastFeatureCollection {
    #[serde(rename = "type")]
    pub collection_type: String,
    pub features: Vec<OblastFeature>,
}

/// Ukrainian oblast names mapping
pub fn get_ukrainian_oblast_names() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Vinnytsia", "Вінницька область"),
        ("Volyn", "Волинська область"),
        ("Dnipropetrovsk", "Дніпропетровська область"),
        ("Donetsk", "Донецька область"),
        ("Zhytomyr", "Житомирська область"),
        ("Zakarpattia", "Закарпатська область"),
        ("Zaporizhzhia", "Запорізька область"),
        ("Ivano-Frankivsk", "Івано-Франківська область"),
        ("Kyiv", "Київська область"),
        ("Kirovohrad", "Кіровоградська область"),
        ("Luhansk", "Луганська область"),
        ("Lviv", "Львівська область"),
        ("Mykolaiv", "Миколаївська область"),
        ("Odesa", "Одеська область"),
        ("Poltava", "Полтавська область"),
        ("Rivne", "Рівненська область"),
        ("Sumy", "Сумська область"),
        ("Ternopil", "Тернопільська область"),
        ("Kharkiv", "Харківська область"),
        ("Kherson", "Херсонська область"),
        ("Khmelnytskyi", "Хмельницька область"),
        ("Cherkasy", "Черкаська область"),
        ("Chernivtsi", "Чернівецька область"),
        ("Chernihiv", "Чернігівська область"),
        ("Crimea", "Автономна Республіка Крим"),
        ("Sevastopol", "Севастополь"),
        ("Kyiv City", "Київ"),
    ]
}

/// Get Ukrainian name for an oblast by English name
pub fn get_ukrainian_name(english_name: &str) -> String {
    get_ukrainian_oblast_names()
        .into_iter()
        .find(|(en, _)| english_name.to_lowercase().contains(&en.to_lowercase()))
        .map(|(_, ua)| ua.to_string())
        .unwrap_or_else(|| english_name.to_string())
}
