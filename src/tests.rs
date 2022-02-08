use std::collections::{HashMap, HashSet};

use geo_types::Point;
use geojson::GeoJson;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::{Map, Value};

use crate::{jitter, load_zones, scrape_points, Options, Subsample};

#[test]
fn test_sums_match() {
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let input_sums = sum_trips_input("data/od.csv", &["all", "car_driver", "foot"]);

    for max_per_od in [1, 10, 100, 1000] {
        let subpoints = scrape_points("data/road_network.geojson").unwrap();
        let options = Options {
            max_per_od,
            subsample_origin: Subsample::UnweightedPoints(subpoints.clone()),
            subsample_destination: Subsample::UnweightedPoints(subpoints),
            all_key: "all".to_string(),
            origin_key: "geo_code1".to_string(),
            destination_key: "geo_code2".to_string(),
            min_distance_meters: 1.0,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let mut raw_output = Vec::new();
        jitter("data/od.csv", &zones, &mut rng, options, &mut raw_output).unwrap();
        let output = String::from_utf8(raw_output)
            .unwrap()
            .parse::<GeoJson>()
            .unwrap();

        for (column, input_sum) in &input_sums {
            let input_sum = *input_sum;
            let output_sum = sum_trips_output(&output, column);
            let epsilon = 1e-6;
            assert!(
                (input_sum - output_sum).abs() < epsilon,
                "Number of {} trips in input {} and jittered output {} don't match for max_per_od = {}",
                column,
                input_sum,
                output_sum,
                max_per_od
            );
        }
    }
}

#[test]
fn test_different_subpoints() {
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let destination_subpoints = scrape_points("data/schools.geojson").unwrap();
    // Keep a copy of the schools as a set
    let schools: HashSet<_> = destination_subpoints.iter().map(hashify_point).collect();

    let options = Options {
        max_per_od: 1,
        subsample_origin: Subsample::RandomPoints,
        subsample_destination: Subsample::UnweightedPoints(destination_subpoints),
        all_key: "walk".to_string(),
        origin_key: "origin".to_string(),
        destination_key: "destination".to_string(),
        min_distance_meters: 1.0,
    };
    let mut rng = StdRng::seed_from_u64(42);
    let mut raw_output = Vec::new();
    jitter(
        "data/od_schools.csv",
        &zones,
        &mut rng,
        options,
        &mut raw_output,
    )
    .unwrap();
    let output = String::from_utf8(raw_output)
        .unwrap()
        .parse::<GeoJson>()
        .unwrap();

    // Verify that all destinations match one of the schools
    if let GeoJson::FeatureCollection(ref fc) = output {
        for feature in &fc.features {
            if let Some(geojson::Value::LineString(ls)) =
                feature.geometry.as_ref().map(|geom| &geom.value)
            {
                let pt = ls.last().unwrap();
                if !schools.contains(&hashify_point(&Point::new(pt[0], pt[1]))) {
                    panic!(
                        "An output feature doesn't end at a school subpoint: {:?}",
                        feature
                    );
                }
            } else {
                panic!("Output geometry isn't a LineString: {:?}", feature.geometry);
            }
        }
    } else {
        panic!("Output isn't a FeatureCollection: {:?}", output);
    }

    // Also make sure sums match, so rows are preserved properly. This input data has 0 for some
    // all_key rows. (Ideally this would be a separate test)
    let input_sums = sum_trips_input("data/od_schools.csv", &["walk", "bike", "other", "car"]);
    for (column, input_sum) in input_sums {
        let output_sum = sum_trips_output(&output, &column);
        let epsilon = 1e-6;
        assert!(
            (input_sum - output_sum).abs() < epsilon,
            "Number of {} trips in input {} and jittered output {} don't match",
            column,
            input_sum,
            output_sum,
        );
    }
}

// TODO Test zone names that look numeric and contain leading 0's

fn sum_trips_input(csv_path: &str, keys: &[&str]) -> HashMap<String, f64> {
    let mut totals = HashMap::new();
    for key in keys {
        totals.insert(key.to_string(), 0.0);
    }
    for rec in csv::Reader::from_path(csv_path).unwrap().deserialize() {
        let map: Map<String, Value> = rec.unwrap();
        for key in keys {
            if let Value::Number(x) = &map[*key] {
                // or_insert is redundant
                let total = totals.entry(key.to_string()).or_insert(0.0);
                *total += x.as_f64().unwrap();
            }
        }
    }
    totals
}

// TODO Refactor helpers -- probably also return a HashMap here
fn sum_trips_output(gj: &GeoJson, all_key: &str) -> f64 {
    let mut total = 0.0;
    if let GeoJson::FeatureCollection(fc) = gj {
        for feature in &fc.features {
            total += feature.property(all_key).unwrap().as_f64().unwrap();
        }
    }
    total
}

// Since f64 isn't hashable, just round to 3 decimal places for comparisons
fn hashify_point(pt: &Point<f64>) -> Point<i64> {
    Point::new((pt.x() * 1000.0) as i64, (pt.y() * 1000.0) as i64)
}
