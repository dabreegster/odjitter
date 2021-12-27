use std::collections::HashMap;
use std::fs::File;

use anyhow::Result;
use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::contains::Contains;
use geo_types::{LineString, MultiPolygon, Point};
use geojson::GeoJson;
use rand::prelude::SliceRandom;
use rand::rngs::StdRng;
use rand::Rng;

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

pub fn jitter(
    csv_path: &str,
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

    for rec in csv::Reader::from_reader(File::open(csv_path)?).deserialize() {
        let mut key_value: HashMap<String, String> = rec?;
        let origin_id = key_value[&options.origin_key].clone();
        let destination_id = key_value[&options.destination_key].clone();

        // How many times will we jitter this one row?
        let repeat =
            (key_value[&options.all_key].parse::<f64>()? / (options.max_per_od as f64)).ceil();

        // Scale all of the numeric values
        for value in key_value.values_mut() {
            if let Ok(x) = value.parse::<f64>() {
                *value = (x / repeat).to_string();
            }
        }

        if let Some(ref points) = points_per_zone {
            let points_in_o = &points[&origin_id];
            let points_in_d = &points[&destination_id];
            for _ in 0..repeat as usize {
                // TODO If a zone has no subpoints, fail -- bad input. Be clear about that.
                // TODO Sample with replacement or not?
                // TODO Make sure o != d
                let o = *points_in_o.choose(rng).unwrap();
                let d = *points_in_d.choose(rng).unwrap();
                output.push((vec![o, d].into(), key_value.clone()));
            }
        } else {
            let origin_polygon = &zones[&origin_id];
            let destination_polygon = &zones[&destination_id];
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

fn convert_to_geojson(input: Vec<(LineString<f64>, HashMap<String, String>)>) -> GeoJson {
    let geom_collection: geo::GeometryCollection<f64> =
        input.iter().map(|(geom, _)| geom.clone()).collect();
    let mut feature_collection = geojson::FeatureCollection::from(&geom_collection);
    for (feature, (_, key_value)) in feature_collection.features.iter_mut().zip(input) {
        let mut properties = serde_json::Map::new();
        // TODO Preserve csv order
        for (k, v) in key_value {
            if let Ok(numeric) = v.parse::<f64>() {
                // TODO Skip geocode1 and the special fields
                // If it's numeric, express it that way in JSON
                properties.insert(k, numeric.into());
            } else {
                // It's a string, let it be one in JSON
                properties.insert(k, v.into());
            }
        }
        feature.properties = Some(properties);
    }
    GeoJson::from(feature_collection)
}
