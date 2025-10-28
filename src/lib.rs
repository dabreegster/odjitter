//! This crate transforms origin/destination data aggregated by zone into a disaggregated form, by
//! sampling specific points from the zone.
//!
//! TODO: Motivate and explain with a full example.

mod scrape;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::BufReader;
use std::path::Path;

use anyhow::{bail, Result};
use fs_err::File;
use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::contains::Contains;
use geo::algorithm::haversine_distance::HaversineDistance;
use geo_types::{LineString, MultiPolygon, Point, Rect};
use geojson::{Feature, FeatureReader};
use ordered_float::NotNan;
use rand::prelude::SliceRandom;
use rand::rngs::StdRng;
use rand::Rng;
use rstar::{RTree, RTreeObject, AABB};
use serde_json::{Map, Value};

pub use self::scrape::scrape_points;

pub struct Options {
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
    /// Prevent duplicate (origin, destination) pairs from appearing in the output. This may
    /// increase memory and runtime requirements. Note the duplication uses the floating point
    /// precision of the input data, and only consider geometry (not any properties).
    pub deduplicate_pairs: bool,
}

/// Specifies how specific points should be generated within a zone.
pub enum Subsample {
    /// Pick points uniformly at random within the zone's shape.
    ///
    /// Note that "within" excludes points directly on the zone's boundary.
    RandomPoints,
    /// Sample from points within the zone's shape, where each point has a relative weight.
    ///
    /// Note that "within" excludes points directly on the zone's boundary. If a point lies in more
    /// than one zone, it'll be assigned to any of those zones arbitrarily. (This means the input
    /// zones overlap.)
    WeightedPoints(Vec<WeightedPoint>),
}

/// A point with an associated relative weight. Higher weights are more likely to be sampled.
#[derive(Clone)]
pub struct WeightedPoint {
    pub point: Point<f64>,
    pub weight: f64,
}

impl RTreeObject for WeightedPoint {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point([self.point.x(), self.point.y()])
    }
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
/// The output LineStrings are provided by callback.
///
/// Note this assumes assumes all input is in the WGS84 coordinate system, and uses the Haversine
/// formula to calculate distances.
///
/// # Arguments
///
/// * `disaggregation_threshold` - What's the maximum number of trips per output OD row that's
///   allowed? If an input OD row contains less than this, it will appear in the output without
///   transformation. Otherwise, the input row is repeated until the sum matches the original value,
///   but each output row obeys this maximum.
/// * `disaggregation_key` - Which column in the OD row specifies the total number of trips to
///   disaggregate?
pub fn jitter<P: AsRef<Path>, F: FnMut(Feature) -> Result<()>>(
    csv_path: P,
    zones: &HashMap<String, MultiPolygon<f64>>,
    disaggregation_threshold: usize,
    disaggregation_key: String,
    rng: &mut StdRng,
    options: Options,
    mut output: F,
) -> Result<()> {
    // TODO Don't allow disaggregation_threshold to be 0
    let csv_path = csv_path.as_ref();

    let points_per_origin_zone: Option<BTreeMap<String, Vec<WeightedPoint>>> =
        if let Subsample::WeightedPoints(points) = options.subsample_origin {
            Some(points_per_polygon(points, zones))
        } else {
            None
        };
    let points_per_destination_zone: Option<BTreeMap<String, Vec<WeightedPoint>>> =
        if let Subsample::WeightedPoints(points) = options.subsample_destination {
            Some(points_per_polygon(points, zones))
        } else {
            None
        };

    let mut seen_pairs: HashSet<ODPair> = HashSet::new();

    println!("Disaggregating OD data");
    for rec in csv::Reader::from_reader(File::open(csv_path)?).deserialize() {
        // It's tempting to deserialize directly into a serde_json::Map<String, Value> and
        // auto-detect strings and numbers. But sadly, some input data has zone names that look
        // numeric, and even contain leading zeros, which'll be lost. So first just grab raw
        // strings
        let string_map: HashMap<String, String> = rec?;

        // How many times will we jitter this one row?
        let repeat = if let Some(count) = string_map
            .get(&disaggregation_key)
            .and_then(|count| count.parse::<f64>().ok())
        {
            // If disaggregation_key is 0 for this row, don't scale the counts, but still preserve
            // the row (and jitter it just once)
            if count == 0.0 {
                1.0
            } else {
                (count / disaggregation_threshold as f64).ceil()
            }
        } else {
            bail!(
                "{} doesn't have a {} column or the value isn't numeric; set disaggregation_key properly",
                csv_path.display(),
                disaggregation_key
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

        let origin_zone = if let Some(zone) = zones.get(origin_id) {
            zone
        } else {
            bail!("Unknown origin zone {origin_id}");
        };
        let destination_zone = if let Some(zone) = zones.get(destination_id) {
            zone
        } else {
            bail!("Unknown destination zone {destination_id}");
        };
        let origin_sampler = Subsampler::new(&points_per_origin_zone, origin_zone, origin_id)?;
        let destination_sampler = Subsampler::new(
            &points_per_destination_zone,
            destination_zone,
            destination_id,
        )?;

        if options.deduplicate_pairs {
            if let (Some(num_origin), Some(num_destination)) = (
                origin_sampler.num_points(),
                destination_sampler.num_points(),
            ) {
                if repeat as usize > num_origin * num_destination {
                    bail!("{repeat} unique pairs requested from {origin_id} ({num_origin} subpoints) to {destination_id} ({num_destination} subpoints), but this is impossible");
                }
            }
        }

        for _ in 0..repeat as usize {
            loop {
                let o = origin_sampler.sample(rng);
                let d = destination_sampler.sample(rng);
                if o.haversine_distance(&d) >= options.min_distance_meters {
                    if options.deduplicate_pairs {
                        let pair = hashify(o, d);
                        if seen_pairs.contains(&pair) {
                            continue;
                        } else {
                            seen_pairs.insert(pair);
                        }
                    }

                    output(to_geojson(o, d, json_map.clone()))?;
                    break;
                }
            }
        }
    }
    Ok(())
}

/// This method transforms aggregate origin/destination pairs into a fully disaggregated form, by
/// sampling specific points from the zone.
///
/// The input is a CSV file, with each row representing trips between an origin and destination,
/// expressed as a named zone. All numeric columns in the CSV file are interpreted as a number of
/// trips by different modes (like walking, cycling, etc).
///
/// Each input row is repeated some number of times, based on the counts in each mode column. The
/// output will have a new `mode` column set to that.
///
/// The output LineStrings are provided by callback.
///
/// Note this assumes assumes all input is in the WGS84 coordinate system, and uses the Haversine
/// formula to calculate distances.
///
pub fn disaggregate<P: AsRef<Path>, F: FnMut(Feature) -> Result<()>>(
    csv_path: P,
    zones: &HashMap<String, MultiPolygon<f64>>,
    rng: &mut StdRng,
    options: Options,
    mut output: F,
) -> Result<()> {
    let csv_path = csv_path.as_ref();

    let points_per_origin_zone: Option<BTreeMap<String, Vec<WeightedPoint>>> =
        if let Subsample::WeightedPoints(points) = options.subsample_origin {
            Some(points_per_polygon(points, zones))
        } else {
            None
        };
    let points_per_destination_zone: Option<BTreeMap<String, Vec<WeightedPoint>>> =
        if let Subsample::WeightedPoints(points) = options.subsample_destination {
            Some(points_per_polygon(points, zones))
        } else {
            None
        };

    println!("Disaggregating OD data");
    for rec in csv::Reader::from_reader(File::open(csv_path)?).deserialize() {
        // It's tempting to deserialize directly into a serde_json::Map<String, Value> and
        // auto-detect strings and numbers. But sadly, some input data has zone names that look
        // numeric, and even contain leading zeros, which'll be lost. So first just grab raw
        // strings
        let mut string_map: HashMap<String, String> = rec?;

        let origin_id = if let Some(id) = string_map.remove(&options.origin_key) {
            id
        } else {
            bail!(
                "{} doesn't have a {} column; set origin_key properly",
                csv_path.display(),
                options.origin_key
            );
        };
        let destination_id = if let Some(id) = string_map.remove(&options.destination_key) {
            id
        } else {
            bail!(
                "{} doesn't have a {} column; set destination_key properly",
                csv_path.display(),
                options.destination_key
            );
        };
        let origin_zone = if let Some(zone) = zones.get(&origin_id) {
            zone
        } else {
            bail!("Unknown origin zone {origin_id}");
        };
        let destination_zone = if let Some(zone) = zones.get(&destination_id) {
            zone
        } else {
            bail!("Unknown destination zone {destination_id}");
        };
        let origin_sampler = Subsampler::new(&points_per_origin_zone, origin_zone, &origin_id)?;
        let destination_sampler = Subsampler::new(
            &points_per_destination_zone,
            destination_zone,
            &destination_id,
        )?;

        let mut seen_pairs: HashSet<ODPair> = HashSet::new();

        // Interpret all columns except origin_key and destination_key as numeric, split by mode
        for (mode, value) in string_map {
            if let Ok(count) = value.parse::<f64>() {
                // TODO How should we treat fractional input?
                let count = count as usize;

                if options.deduplicate_pairs {
                    if let (Some(num_origin), Some(num_destination)) = (
                        origin_sampler.num_points(),
                        destination_sampler.num_points(),
                    ) {
                        if count > num_origin * num_destination {
                            bail!("{count} unique pairs requested for {mode} from {origin_id} ({num_origin} subpoints) to {destination_id} ({num_destination} subpoints), but this is impossible");
                        }
                    }
                }

                for _ in 0..count {
                    loop {
                        let o = origin_sampler.sample(rng);
                        let d = destination_sampler.sample(rng);
                        if o.haversine_distance(&d) >= options.min_distance_meters {
                            if options.deduplicate_pairs {
                                let pair = hashify(o, d);
                                if seen_pairs.contains(&pair) {
                                    continue;
                                } else {
                                    seen_pairs.insert(pair);
                                }
                            }

                            let mut json_map: Map<String, Value> = Map::new();
                            json_map.insert("mode".to_string(), Value::String(mode.clone()));
                            output(to_geojson(o, d, json_map))?;
                            break;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Extract multipolygon zones from a GeoJSON file, using the provided `name_key` as the key in the
/// resulting map.
pub fn load_zones(
    geojson_path: &str,
    name_key: &str,
) -> Result<HashMap<String, MultiPolygon<f64>>> {
    let reader = FeatureReader::from_reader(BufReader::new(File::open(geojson_path)?));
    let mut zones: HashMap<String, MultiPolygon<f64>> = HashMap::new();
    for feature in reader.features() {
        let feature = feature?;
        if let Some(zone_name) = feature
            .property(name_key)
            .and_then(|x| x.as_str())
            .map(|x| x.to_string())
        {
            let gj_geom: geojson::Geometry = feature.geometry.unwrap();
            let geo_geometry: geo_types::Geometry<f64> = gj_geom.try_into().unwrap();
            if let geo_types::Geometry::MultiPolygon(mp) = geo_geometry {
                zones.insert(zone_name, mp);
            } else if let geo_types::Geometry::Polygon(p) = geo_geometry {
                zones.insert(zone_name, p.into());
            }
        } else {
            bail!(
                "Feature doesn't have a string zone name {}: {:?}",
                name_key,
                feature
            );
        }
    }
    Ok(zones)
}

// TODO Share with rampfs
fn points_per_polygon(
    points: Vec<WeightedPoint>,
    polygons: &HashMap<String, MultiPolygon<f64>>,
) -> BTreeMap<String, Vec<WeightedPoint>> {
    let tree = RTree::bulk_load(points);

    let mut output = BTreeMap::new();
    for (key, polygon) in polygons {
        let mut pts_inside = Vec::new();
        let bounds = polygon.bounding_rect().unwrap();
        let min = bounds.min();
        let max = bounds.max();
        let envelope: AABB<[f64; 2]> = AABB::from_corners([min.x, min.y], [max.x, max.y]);
        for pt in tree.locate_in_envelope(&envelope) {
            if polygon.contains(&pt.point) {
                pts_inside.push(pt.clone());
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
    WeightedPoints(&'a Vec<WeightedPoint>),
}

impl<'a> Subsampler<'a> {
    fn new(
        points_per_zone: &'a Option<BTreeMap<String, Vec<WeightedPoint>>>,
        zone_polygon: &'a MultiPolygon<f64>,
        zone_id: &str,
    ) -> Result<Subsampler<'a>> {
        if let Some(points_per_zone) = points_per_zone {
            if let Some(points) = points_per_zone.get(zone_id) {
                if !points.is_empty() {
                    return Ok(Subsampler::WeightedPoints(points));
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
            Subsampler::WeightedPoints(points) => {
                // TODO Sample with replacement or not?
                // TODO If there are no two subpoints that're greater than this distance, we'll
                // infinite loop. Detect upfront, or maybe just give up after a fixed number of
                // attempts?
                points.choose_weighted(rng, |pt| pt.weight).unwrap().point
            }
        }
    }

    /// No result for random points in a polygon (infinite, unless the polygon is extremely
    /// degenerate). For weighted points, returns the number of them.
    fn num_points(&self) -> Option<usize> {
        match self {
            Subsampler::RandomPoints(_, _) => None,
            Subsampler::WeightedPoints(points) => Some(points.len()),
        }
    }
}

type ODPair = [NotNan<f64>; 4];
fn hashify(o: Point<f64>, d: Point<f64>) -> ODPair {
    // We can't collect into an array, so write this a bit manually
    [
        NotNan::new(o.x()).unwrap(),
        NotNan::new(o.y()).unwrap(),
        NotNan::new(d.x()).unwrap(),
        NotNan::new(d.y()).unwrap(),
    ]
}
