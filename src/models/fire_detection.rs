use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

/// Fire detection time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireDetectionPeriod {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

impl Default for FireDetectionPeriod {
    fn default() -> Self {
        let today = chrono::Local::now().date_naive();
        let start = today - Duration::days(30);
        Self {
            start_date: start,
            end_date: today,
        }
    }
}

impl FireDetectionPeriod {
    pub fn new(start: NaiveDate, end: NaiveDate) -> Self {
        Self {
            start_date: start,
            end_date: end,
        }
    }

    pub fn days(&self) -> i64 {
        (self.end_date - self.start_date).num_days()
    }

    pub fn format_range(&self) -> String {
        format!(
            "{} - {}",
            self.start_date.format("%d.%m.%Y"),
            self.end_date.format("%d.%m.%Y")
        )
    }
}

/// Comparison imagery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonImageryConfig {
    /// Include imagery from previous months
    pub include_previous_months: bool,
    /// Number of previous months to include (1-12)
    pub previous_months_count: u8,
    /// Include same months from previous year
    pub include_previous_year: bool,
    /// How many previous years to include
    pub previous_years_count: u8,
}

impl Default for ComparisonImageryConfig {
    fn default() -> Self {
        Self {
            include_previous_months: true,
            previous_months_count: 2,
            include_previous_year: true,
            previous_years_count: 1,
        }
    }
}

impl ComparisonImageryConfig {
    /// Generate all comparison periods based on the fire detection period
    pub fn generate_comparison_periods(
        &self,
        fire_period: &FireDetectionPeriod,
    ) -> Vec<ComparisonPeriod> {
        let mut periods = Vec::new();

        // Add previous months
        if self.include_previous_months && self.previous_months_count > 0 {
            for month_offset in 1..=self.previous_months_count {
                if let Some(period) =
                    self.create_previous_month_period(fire_period, month_offset as i32)
                {
                    periods.push(period);
                }
            }
        }

        // Add same months from previous year(s)
        if self.include_previous_year && self.previous_years_count > 0 {
            for year_offset in 1..=self.previous_years_count {
                if let Some(period) =
                    self.create_previous_year_period(fire_period, year_offset as i32)
                {
                    periods.push(period);
                }
            }
        }

        periods
    }

    fn create_previous_month_period(
        &self,
        fire_period: &FireDetectionPeriod,
        month_offset: i32,
    ) -> Option<ComparisonPeriod> {
        let start = subtract_months(fire_period.start_date, month_offset)?;
        let end = subtract_months(fire_period.end_date, month_offset)?;

        Some(ComparisonPeriod {
            period: FireDetectionPeriod::new(start, end),
            period_type: ComparisonPeriodType::PreviousMonth(month_offset as u8),
            description: format!(
                "{} місяц{} тому",
                month_offset,
                if month_offset == 1 { "ь" } else { "і" }
            ),
        })
    }

    fn create_previous_year_period(
        &self,
        fire_period: &FireDetectionPeriod,
        year_offset: i32,
    ) -> Option<ComparisonPeriod> {
        let start = fire_period
            .start_date
            .with_year(fire_period.start_date.year() - year_offset)?;
        let end = fire_period
            .end_date
            .with_year(fire_period.end_date.year() - year_offset)?;

        Some(ComparisonPeriod {
            period: FireDetectionPeriod::new(start, end),
            period_type: ComparisonPeriodType::SameMonthPreviousYear(year_offset as u8),
            description: format!(
                "Ті ж місяці {} р{} тому",
                year_offset,
                if year_offset == 1 { "ік" } else { "оки" }
            ),
        })
    }
}

/// Helper to subtract months from a date
fn subtract_months(date: NaiveDate, months: i32) -> Option<NaiveDate> {
    let total_months = date.year() * 12 + date.month() as i32 - 1 - months;
    let new_year = total_months / 12;
    let new_month = (total_months % 12 + 1) as u32;

    // Handle day overflow (e.g., Jan 31 -> Feb 28)
    let max_day = days_in_month(new_year, new_month);
    let day = date.day().min(max_day);

    NaiveDate::from_ymd_opt(new_year, new_month, day)
}

/// Get number of days in a month
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Type of comparison period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonPeriodType {
    /// N months before the fire period
    PreviousMonth(u8),
    /// Same months in N years ago
    SameMonthPreviousYear(u8),
}

/// A comparison period for imagery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonPeriod {
    pub period: FireDetectionPeriod,
    pub period_type: ComparisonPeriodType,
    pub description: String,
}

impl ComparisonPeriod {
    pub fn folder_suffix(&self) -> String {
        match self.period_type {
            ComparisonPeriodType::PreviousMonth(n) => format!("prev_{}_months", n),
            ComparisonPeriodType::SameMonthPreviousYear(n) => format!("prev_{}_year", n),
        }
    }
}

/// NASA FIRMS fire detection data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirePoint {
    /// Latitude
    pub latitude: f64,
    /// Longitude
    pub longitude: f64,
    /// Brightness temperature (Kelvin)
    pub brightness: f64,
    /// Acquisition date
    pub acq_date: NaiveDate,
    /// Acquisition time (HHMM)
    pub acq_time: String,
    /// Satellite source (e.g., "VIIRS", "MODIS")
    pub satellite: String,
    /// Fire radiative power (MW)
    pub frp: Option<f64>,
    /// Confidence level
    pub confidence: String,
    /// Day/Night flag
    pub daynight: Option<String>,
}

impl FirePoint {
    /// Create a bounding box around the fire point
    /// Default size is approximately 1km buffer
    pub fn bounding_box(&self, buffer_km: f64) -> [f64; 4] {
        // Approximate degrees per km at this latitude
        let lat_deg_per_km = 1.0 / 111.0;
        let lon_deg_per_km = 1.0 / (111.0 * self.latitude.to_radians().cos());

        let lat_buffer = buffer_km * lat_deg_per_km;
        let lon_buffer = buffer_km * lon_deg_per_km;

        [
            self.longitude - lon_buffer, // min_lon
            self.latitude - lat_buffer,  // min_lat
            self.longitude + lon_buffer, // max_lon
            self.latitude + lat_buffer,  // max_lat
        ]
    }
}

/// Fire cluster (group of nearby fire points)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireCluster {
    pub id: String,
    pub points: Vec<FirePoint>,
    pub bbox: [f64; 4],
    pub centroid: (f64, f64),
    pub earliest_date: NaiveDate,
    pub latest_date: NaiveDate,
}

impl FireCluster {
    pub fn from_points(id: String, points: Vec<FirePoint>) -> Option<Self> {
        if points.is_empty() {
            return None;
        }

        let mut min_lon = f64::MAX;
        let mut min_lat = f64::MAX;
        let mut max_lon = f64::MIN;
        let mut max_lat = f64::MIN;
        let mut sum_lon = 0.0;
        let mut sum_lat = 0.0;
        let mut earliest = points[0].acq_date;
        let mut latest = points[0].acq_date;

        for point in &points {
            min_lon = min_lon.min(point.longitude);
            min_lat = min_lat.min(point.latitude);
            max_lon = max_lon.max(point.longitude);
            max_lat = max_lat.max(point.latitude);
            sum_lon += point.longitude;
            sum_lat += point.latitude;
            if point.acq_date < earliest {
                earliest = point.acq_date;
            }
            if point.acq_date > latest {
                latest = point.acq_date;
            }
        }

        let n = points.len() as f64;
        Some(Self {
            id,
            bbox: [min_lon, min_lat, max_lon, max_lat],
            centroid: (sum_lon / n, sum_lat / n),
            earliest_date: earliest,
            latest_date: latest,
            points,
        })
    }

    /// Expand bbox by buffer in km
    pub fn expanded_bbox(&self, buffer_km: f64) -> [f64; 4] {
        let center_lat = (self.bbox[1] + self.bbox[3]) / 2.0;
        let lat_deg_per_km = 1.0 / 111.0;
        let lon_deg_per_km = 1.0 / (111.0 * center_lat.to_radians().cos());

        let lat_buffer = buffer_km * lat_deg_per_km;
        let lon_buffer = buffer_km * lon_deg_per_km;

        [
            self.bbox[0] - lon_buffer,
            self.bbox[1] - lat_buffer,
            self.bbox[2] + lon_buffer,
            self.bbox[3] + lat_buffer,
        ]
    }
}
