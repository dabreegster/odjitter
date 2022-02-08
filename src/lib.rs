//! This crate transforms origin/destination data aggregated by zone into a disaggregated form, by
//! sampling specific points from the zone.
//!
//! TODO: Motivate and explain with a full example.

mod scrape;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Result};
use fs_err::File;
use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::contains::Contains;
use geo::algorithm::haversine_distance::HaversineDistance;
use geo_types::{LineString, MultiPolygon, Point, Rect};
use geojson::{Feature, GeoJson};
use rand::prelude::SliceRandom;
use rand::rngs::StdRng;
use rand::Rng;
use rstar::{RTree, AABB};
use serde_json::{Map, Value};

pub use self::scrape::scrape_points;

// TODO Use bufreaders/writers (but measure perf first)

// TODO Docs
// TODO Setup github builds

pub struct Options {
    /// What's the maximum number of trips per output OD row that's allowed? If an input OD row
    /// contains less than this, it will appear in the output without transformation. Otherwise,
    /// the input row is repeated until the sum matches the original value, but each output row
    /// obeys this maximum.
    ///
    /// TODO Don't allow this to be 0
    /// TODO If this is 1, it'd be more natural to set one "mode" column
    pub disaggregation_threshold: usize,
    /// Which column in the OD row specifies the total number of trips to disaggregate?
    pub disaggregation_key: String,
    /// How to pick points from origin zones
    pub subsample_origin: Subsample,
    /// How to pick points from destination zones
    pub subsample_destination: Subsample,
    /// Which column in the OD row specifies the zone where trips originate?
    pub origin_key: String,
    /// Which column in the OD row specifies the zone where trips ends?
    pub destination_key: String,
    /// Guarantee that jittered points are at least this distance apart.
    pub min_distance_meters: f64,
}

/// Specifies how specific points should be generated within a zone.
pub enum Subsample {
    /// Pick points uniformly at random within the zone's shape.
    ///
    /// Note that "within" excludes points directly on the zone's boundary.
    RandomPoints,
    /// Sample uniformly at random from these points within the zone's shape.
    ///
    /// Note that "within" excludes points directly on the zone's boundary. If a point lies in more
    /// than one zone, it'll be assigned to any of those zones arbitrarily. (This means the input
    /// zones overlap.)
    UnweightedPoints(Vec<Point<f64>>),
}

/// This method transforms aggregate origin/destination pairs into a disaggregated form, by
/// sampling specific points from the zone.
///
/// The input is a CSV file, with each row representing trips between an origin and destination,
/// expressed as a named zone. The columns in the CSV file can break down the number of trips by
/// different modes (like walking, cycling, etc).
///
/// Each input row is repeated some number of times, based on `disaggregation_threshold`. If the
/// row originally represents 100 trips and `disaggregation_threshold` is 5, then the row will be
/// repeated 20 times. Each time, the origin and destination will be transformed from the entire
/// zone to a specific point within the zone, determined using the specified `Subsample`.
///
/// The output is written as GeoJSON to the provided writer.
///
/// Note this assumes assumes all input is in the WGS84 coordinate system, and uses the Haversine
/// formula to calculate distances.
pub fn jitter<P: AsRef<Path>, W: Write>(
    csv_path: P,
    zones: &HashMap<String, MultiPolygon<f64>>,
    rng: &mut StdRng,
    options: Options,
    mut writer: W,
) -> Result<()> {
    let csv_path = csv_path.as_ref();

    let points_per_origin_zone: Option<BTreeMap<String, Vec<Point<f64>>>> =
        if let Subsample::UnweightedPoints(points) = options.subsample_origin {
            Some(points_per_polygon(points, zones))
        } else {
            None
        };
    let points_per_destination_zone: Option<BTreeMap<String, Vec<Point<f64>>>> =
        if let Subsample::UnweightedPoints(points) = options.subsample_destination {
            Some(points_per_polygon(points, zones))
        } else {
            None
        };

    // Manually write GeoJSON, so we can write per feature, instead of collecting the whole
    // FeatureCollection in memory
    writeln!(writer, "{{\"type\":\"FeatureCollection\", \"features\":[")?;
    let mut add_comma = false;

    println!("Disaggregating OD data");
    for rec in csv::Reader::from_reader(File::open(csv_path)?).deserialize() {
        // It's tempting to deserialize directly into a serde_json::Map<String, Value> and
        // auto-detect strings and numbers. But sadly, some input data has zone names that look
        // numeric, and even contain leading zeros, which'll be lost. So first just grab raw
        // strings
        let string_map: HashMap<String, String> = rec?;

        // How many times will we jitter this one row?
        let repeat = if let Some(count) = string_map
            .get(&options.disaggregation_key)
            .and_then(|count| count.parse::<f64>().ok())
        {
            // If disaggregation_key is 0 for this row, don't scale the counts, but still preserve
            // the row (and jitter it just once)
            if count == 0.0 {
                1.0
            } else {
                (count / options.disaggregation_threshold as f64).ceil()
            }
        } else {
            bail!(
                "{} doesn't have a {} column or the value isn't numeric; set disaggregation_key properly",
                csv_path.display(),
                options.disaggregation_key
            );
        };

        // Transform to a JSON map
        let mut json_map: Map<String, Value> = Map::new();
        for (key, value) in string_map {
            let json_value = if key == options.origin_key || key == options.destination_key {
                // Never treat the origin/destination key as numeric
                Value::String(value)
            } else if let Ok(x) = value.parse::<f64>() {
                // Scale all of the numeric values. Note the unwrap is safe -- we should never wind
                // up with NaN or infinity
                Value::Number(serde_json::Number::from_f64(x / repeat).unwrap())
            } else {
                Value::String(value)
            };
            json_map.insert(key, json_value);
        }

        let origin_id = if let Some(Value::String(id)) = json_map.get(&options.origin_key) {
            id
        } else {
            bail!(
                "{} doesn't have a {} column; set origin_key properly",
                csv_path.display(),
                options.origin_key
            );
        };
        let destination_id = if let Some(Value::String(id)) = json_map.get(&options.destination_key)
        {
            id
        } else {
            bail!(
                "{} doesn't have a {} column; set destination_key properly",
                csv_path.display(),
                options.destination_key
            );
        };

        let origin_sampler =
            Subsampler::new(&points_per_origin_zone, &zones[origin_id], origin_id)?;
        let destination_sampler = Subsampler::new(
            &points_per_destination_zone,
            &zones[destination_id],
            destination_id,
        )?;

        for _ in 0..repeat as usize {
            loop {
                let o = origin_sampler.sample(rng);
                let d = destination_sampler.sample(rng);
                if o.haversine_distance(&d) >= options.min_distance_meters {
                    if add_comma {
                        writeln!(writer, ",")?;
                    } else {
                        add_comma = true;
                    }
                    serde_json::to_writer(&mut writer, &to_geojson(o, d, json_map.clone()))?;
                    break;
                }
            }
        }
    }
    writeln!(writer, "]}}")?;
    Ok(())
}

/// Extract multipolygon zones from a GeoJSON file, using the provided `name_key` as the key in the
/// resulting map.
pub fn load_zones(
    geojson_path: &str,
    name_key: &str,
) -> Result<HashMap<String, MultiPolygon<f64>>> {
    let geojson_input = fs_err::read_to_string(geojson_path)?;
    let geojson = geojson_input.parse::<GeoJson>()?;

    let mut zones: HashMap<String, MultiPolygon<f64>> = HashMap::new();
    if let GeoJson::FeatureCollection(collection) = geojson {
        for feature in collection.features {
            if let Some(zone_name) = feature
                .property(name_key)
                .and_then(|x| x.as_str())
                .map(|x| x.to_string())
            {
                let gj_geom: geojson::Geometry = feature.geometry.unwrap();
                let geo_geometry: geo_types::Geometry<f64> = gj_geom.try_into().unwrap();
                // TODO Support polygons too
                if let geo_types::Geometry::MultiPolygon(mp) = geo_geometry {
                    zones.insert(zone_name, mp);
                }
            } else {
                bail!(
                    "Feature doesn't have a string zone name {}: {:?}",
                    name_key,
                    feature
                );
            }
        }
    }
    Ok(zones)
}

// TODO Share with rampfs
fn points_per_polygon(
    points: Vec<Point<f64>>,
    polygons: &HashMap<String, MultiPolygon<f64>>,
) -> BTreeMap<String, Vec<Point<f64>>> {
    let tree = RTree::bulk_load(points);

    let mut output = BTreeMap::new();
    for (key, polygon) in polygons {
        let mut pts_inside = Vec::new();
        let bounds = polygon.bounding_rect().unwrap();
        let envelope: AABB<Point<f64>> =
            AABB::from_corners(bounds.min().into(), bounds.max().into());
        for pt in tree.locate_in_envelope(&envelope) {
            if polygon.contains(pt) {
                pts_inside.push(*pt);
            }
        }
        output.insert(key.clone(), pts_inside);
    }
    output
}

fn to_geojson(pt1: Point<f64>, pt2: Point<f64>, properties: Map<String, Value>) -> Feature {
    let line_string: LineString<f64> = vec![pt1, pt2].into();
    Feature {
        geometry: Some(geojson::Geometry {
            value: geojson::Value::from(&line_string),
            bbox: None,
            foreign_members: None,
        }),
        properties: Some(properties),
        bbox: None,
        id: None,
        foreign_members: None,
    }
}

enum Subsampler<'a> {
    RandomPoints(&'a MultiPolygon<f64>, Rect<f64>),
    UnweightedPoints(&'a Vec<Point<f64>>),
}

impl<'a> Subsampler<'a> {
    fn new(
        points_per_zone: &'a Option<BTreeMap<String, Vec<Point<f64>>>>,
        zone_polygon: &'a MultiPolygon<f64>,
        zone_id: &str,
    ) -> Result<Subsampler<'a>> {
        if let Some(points_per_zone) = points_per_zone {
            if let Some(points) = points_per_zone.get(zone_id) {
                if !points.is_empty() {
                    return Ok(Subsampler::UnweightedPoints(points));
                }
            }
            bail!("No subpoints for zone {}", zone_id);
        } else {
            match zone_polygon.bounding_rect() {
                Some(bounds) => Ok(Subsampler::RandomPoints(zone_polygon, bounds)),
                None => bail!("can't calculate bounding box for zone {}", zone_id),
            }
        }
    }

    fn sample(&self, rng: &mut StdRng) -> Point<f64> {
        match self {
            Subsampler::RandomPoints(polygon, bounds) => loop {
                let x = rng.gen_range(bounds.min().x..=bounds.max().x);
                let y = rng.gen_range(bounds.min().y..=bounds.max().y);
                let pt = Point::new(x, y);
                if polygon.contains(&pt) {
                    return pt;
                }
            },
            Subsampler::UnweightedPoints(points) => {
                // TODO Sample with replacement or not?
                // TODO If there are no two subpoints that're greater than this distance, we'll
                // infinite loop. Detect upfront, or maybe just give up after a fixed number of
                // attempts?
                *points.choose(rng).unwrap()
            }
        }
    }
}
