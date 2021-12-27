use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::contains::Contains;
use geo_types::{LineString, MultiPolygon, Point};
use geojson::GeoJson;
use rand::prelude::SliceRandom;
use rand::rngs::StdRng;
use rand::Rng;
use serde_json::{Map, Value};

// TODO Weighted subpoints
// TODO Grab subpoints from OSM road network
// TODO Grab subpoints from OSM buildings, weighted

pub struct Options {
    /// What's the maximum number of trips per output OD row that's allowed? If an input OD row
    /// contains less than this, it will appear in the output without transformation. Otherwise,
    /// the input row is repeated until the sum matches the original value, but each output row
    /// obeys this maximum.
    pub max_per_od: usize,
    pub subsample: Subsample,
    /// Which column in the OD row specifies the total number of trips to disaggregate?
    pub all_key: String,
    /// Which column in the OD row specifies the zone where trips originate?
    pub origin_key: String,
    /// Which column in the OD row specifies the zone where trips ends?
    pub destination_key: String,
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

pub fn jitter<P: AsRef<Path>>(
    csv_path: P,
    zones: &HashMap<String, MultiPolygon<f64>>,
    rng: &mut StdRng,
    options: Options,
) -> Result<GeoJson> {
    let mut output = Vec::new();

    let points_per_zone: Option<HashMap<String, Vec<Point<f64>>>> =
        if let Subsample::UnweightedPoints(points) = options.subsample {
            Some(points_per_zone(points, zones))
        } else {
            None
        };

    for rec in csv::Reader::from_path(csv_path)?.deserialize() {
        // Transform from CSV directly into a JSON map, auto-detecting strings and numbers.
        // TODO Even if origin_key or destination_key looks like a number, force it into a string
        let mut key_value: Map<String, Value> = rec?;

        // How many times will we jitter this one row?
        let repeat =
            (key_value[&options.all_key].as_f64().unwrap() / (options.max_per_od as f64)).ceil();

        // Scale all of the numeric values
        for (key, value) in &mut key_value {
            // ... but never the zone names, even if they look numeric!
            if key == &options.origin_key || key == &options.destination_key {
                continue;
            }
            if let Some(x) = value.as_f64() {
                // Crashes on NaNs, infinity
                *value = Value::Number(serde_json::Number::from_f64(x / repeat).unwrap());
            }
        }

        let origin_id: &str = key_value[&options.origin_key].as_str().unwrap();
        let destination_id: &str = key_value[&options.destination_key].as_str().unwrap();

        if let Some(ref points) = points_per_zone {
            let points_in_o = &points[origin_id];
            let points_in_d = &points[destination_id];
            for _ in 0..repeat as usize {
                // TODO If a zone has no subpoints, fail -- bad input. Be clear about that.
                // TODO Sample with replacement or not?
                // TODO Make sure o != d
                let o = *points_in_o.choose(rng).unwrap();
                let d = *points_in_d.choose(rng).unwrap();
                output.push((vec![o, d].into(), key_value.clone()));
            }
        } else {
            let origin_polygon = &zones[origin_id];
            let destination_polygon = &zones[destination_id];
            for _ in 0..repeat as usize {
                let o = random_pt(rng, origin_polygon);
                let d = random_pt(rng, destination_polygon);
                output.push((vec![o, d].into(), key_value.clone()));
            }
        }
    }
    Ok(convert_to_geojson(output))
}

pub fn load_zones(
    geojson_path: &str,
    name_key: &str,
) -> Result<HashMap<String, MultiPolygon<f64>>> {
    let geojson_input = std::fs::read_to_string(geojson_path)?;
    let geojson = geojson_input.parse::<GeoJson>()?;

    let mut zones: HashMap<String, MultiPolygon<f64>> = HashMap::new();
    if let geojson::GeoJson::FeatureCollection(collection) = geojson {
        for feature in collection.features {
            let zone_name = feature
                .property(name_key)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
            let gj_geom: geojson::Geometry = feature.geometry.unwrap();
            let geo_geometry: geo_types::Geometry<f64> = gj_geom.try_into().unwrap();
            if let geo_types::Geometry::MultiPolygon(mp) = geo_geometry {
                zones.insert(zone_name, mp);
            }
        }
    }
    Ok(zones)
}

pub fn scrape_points(path: &str) -> Result<Vec<Point<f64>>> {
    let geojson_input = std::fs::read_to_string(path)?;
    let geojson = geojson_input.parse::<GeoJson>()?;
    let mut points = Vec::new();
    if let geojson::GeoJson::FeatureCollection(collection) = geojson {
        for feature in collection.features {
            if let Some(geom) = feature.geometry {
                let geo_geometry: geo_types::Geometry<f64> = geom.try_into().unwrap();
                // TODO Scrape points from all types
                if let geo_types::Geometry::LineString(ls) = geo_geometry {
                    points.extend(ls.into_points());
                }
            }
        }
    }
    Ok(points)
}

fn random_pt(rng: &mut StdRng, poly: &MultiPolygon<f64>) -> Point<f64> {
    let bounds = poly.bounding_rect().unwrap();
    loop {
        let x = rng.gen_range(bounds.min().x..=bounds.max().x);
        let y = rng.gen_range(bounds.min().y..=bounds.max().y);
        let pt = Point::new(x, y);
        if poly.contains(&pt) {
            return pt;
        }
    }
}

fn points_per_zone(
    points: Vec<Point<f64>>,
    zones: &HashMap<String, MultiPolygon<f64>>,
) -> HashMap<String, Vec<Point<f64>>> {
    let mut output = HashMap::new();
    for (name, _) in zones {
        output.insert(name.clone(), Vec::<Point<f64>>::new());
    }
    for point in points {
        for (name, polygon) in zones {
            if polygon.contains(&point) {
                let point_list = output.get_mut(name).unwrap();
                point_list.push(point);
            }
        }
    }
    return output;
}

fn convert_to_geojson(input: Vec<(LineString<f64>, Map<String, Value>)>) -> GeoJson {
    let geom_collection: geo::GeometryCollection<f64> =
        input.iter().map(|(geom, _)| geom.clone()).collect();
    let mut feature_collection = geojson::FeatureCollection::from(&geom_collection);
    for (feature, (_, properties)) in feature_collection.features.iter_mut().zip(input) {
        feature.properties = Some(properties);
    }
    GeoJson::from(feature_collection)
}
