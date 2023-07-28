use std::collections::{HashMap, HashSet};

use geo_types::Point;
use geojson::Feature;
use ordered_float::NotNan;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::{Map, Value};

use crate::{disaggregate, jitter, load_zones, scrape_points, Options, Subsample};

#[test]
fn test_sums_match() {
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let input_sums = sum_trips_input("data/od.csv", &["all", "car_driver", "foot"]);

    for disaggregation_threshold in [1, 10, 100, 1000] {
        let subpoints = scrape_points("data/road_network.geojson", None).unwrap();
        let options = Options {
            subsample_origin: Subsample::WeightedPoints(subpoints.clone()),
            subsample_destination: Subsample::WeightedPoints(subpoints),
            origin_key: "geo_code1".to_string(),
            destination_key: "geo_code2".to_string(),
            min_distance_meters: 1.0,
            deduplicate_pairs: false,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let mut output = Vec::new();
        let disaggregation_key = "all".to_string();
        jitter(
            "data/od.csv",
            &zones,
            disaggregation_threshold,
            disaggregation_key,
            &mut rng,
            options,
            |feature| {
                output.push(feature);
                Ok(())
            },
        )
        .unwrap();

        for (column, input_sum) in &input_sums {
            let input_sum = *input_sum;
            let output_sum = sum_trips_output(&output, column);
            let epsilon = 1e-6;
            assert!(
                (input_sum - output_sum).abs() < epsilon,
                "Number of {} trips in input {} and jittered output {} don't match for disaggregation_threshold = {}",
                column,
                input_sum,
                output_sum,
                disaggregation_threshold
            );
        }
    }
}

#[test]
fn test_different_subpoints() {
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let destination_subpoints =
        scrape_points("data/schools.geojson", Some("weight".to_string())).unwrap();
    // Keep a copy of the schools as a set
    let schools: HashSet<_> = destination_subpoints
        .iter()
        .map(|pt| hashify_point(pt.point))
        .collect();

    let options = Options {
        subsample_origin: Subsample::RandomPoints,
        subsample_destination: Subsample::WeightedPoints(destination_subpoints),
        origin_key: "origin".to_string(),
        destination_key: "destination".to_string(),
        min_distance_meters: 1.0,
        deduplicate_pairs: false,
    };
    let disaggregation_threshold = 1;
    let disaggregation_key = "walk".to_string();
    let mut rng = StdRng::seed_from_u64(42);
    let mut output = Vec::new();
    jitter(
        "data/od_schools.csv",
        &zones,
        disaggregation_threshold,
        disaggregation_key,
        &mut rng,
        options,
        |feature| {
            output.push(feature);
            Ok(())
        },
    )
    .unwrap();

    // Verify that all destinations match one of the schools
    for feature in &output {
        if let Some(geojson::Value::LineString(ls)) =
            feature.geometry.as_ref().map(|geom| &geom.value)
        {
            let pt = ls.last().unwrap();
            if !schools.contains(&hashify_point(Point::new(pt[0], pt[1]))) {
                panic!(
                    "An output feature doesn't end at a school subpoint: {:?}",
                    feature
                );
            }
        } else {
            panic!("Output geometry isn't a LineString: {:?}", feature.geometry);
        }
    }

    // Also make sure sums match, so rows are preserved properly. This input data has 0 for some
    // disaggregation_key rows. (Ideally this would be a separate test)
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

#[test]
fn test_disaggregate() {
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let options = Options {
        subsample_origin: Subsample::RandomPoints,
        subsample_destination: Subsample::RandomPoints,
        origin_key: "geo_code1".to_string(),
        destination_key: "geo_code2".to_string(),
        min_distance_meters: 1.0,
        deduplicate_pairs: false,
    };
    let mut rng = StdRng::seed_from_u64(42);
    let mut output = Vec::new();
    disaggregate("data/od.csv", &zones, &mut rng, options, |feature| {
        output.push(feature);
        Ok(())
    })
    .unwrap();

    // Note "all" has no special meaning to the disaggregate call. The user should probably remove
    // it from the input or ignore it in the output.
    let input_sums = sum_trips_input("data/od.csv", &["all", "car_driver", "foot"]);
    let mut sums_per_mode: HashMap<String, usize> = HashMap::new();
    for feature in output {
        let mode = feature
            .property("mode")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        *sums_per_mode.entry(mode).or_insert(0) += 1;
    }
    for (mode, input_sum) in input_sums {
        let output_sum = sums_per_mode[&mode];
        assert!(
            input_sum as usize == output_sum,
            "Number of {} trips in input {} and disaggregated output {} don't match",
            mode,
            input_sum,
            output_sum,
        );
    }
}

#[test]
fn test_deduplicate_pairs() {
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let subpoints = scrape_points("data/road_network.geojson", None).unwrap();

    for deduplicate_pairs in [false, true] {
        let options = Options {
            subsample_origin: Subsample::WeightedPoints(subpoints.clone()),
            subsample_destination: Subsample::WeightedPoints(subpoints.clone()),
            origin_key: "geo_code1".to_string(),
            destination_key: "geo_code2".to_string(),
            min_distance_meters: 1.0,
            deduplicate_pairs,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let mut output = Vec::new();
        let disaggregation_threshold = 1;
        let disaggregation_key = "all".to_string();
        jitter(
            "data/od.csv",
            &zones,
            disaggregation_threshold,
            disaggregation_key,
            &mut rng,
            options,
            |feature| {
                output.push(feature);
                Ok(())
            },
        )
        .unwrap();

        let mut unique_pairs: HashSet<Vec<NotNan<f64>>> = HashSet::new();

        for feature in &output {
            if let Some(geojson::Value::LineString(ls)) =
                feature.geometry.as_ref().map(|geom| &geom.value)
            {
                unique_pairs.insert(
                    ls.iter()
                        .flatten()
                        .map(|x| NotNan::new(*x).unwrap())
                        .collect(),
                );
            }
        }

        let anything_deduped = output.len() != unique_pairs.len();
        if anything_deduped == deduplicate_pairs {
            panic!(
                "With deduplicate_pairs={}, we got {} LineStrings, with {} unique geometries",
                deduplicate_pairs,
                output.len(),
                unique_pairs.len()
            );
        }
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
fn sum_trips_output(features: &[Feature], disaggregation_key: &str) -> f64 {
    let mut total = 0.0;
    for feature in features {
        total += feature
            .property(disaggregation_key)
            .unwrap()
            .as_f64()
            .unwrap();
    }
    total
}

fn hashify_point(pt: Point<f64>) -> Point<NotNan<f64>> {
    Point::new(NotNan::new(pt.x()).unwrap(), NotNan::new(pt.y()).unwrap())
}
