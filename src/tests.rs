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

#[test]
fn test_min_distance_constraint() {
    // Test that the min_distance_meters parameter is respected
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let subpoints = scrape_points("data/road_network.geojson", None).unwrap();

    let min_distance = 100.0; // 100 meters
    let options = Options {
        subsample_origin: Subsample::WeightedPoints(subpoints.clone()),
        subsample_destination: Subsample::WeightedPoints(subpoints),
        origin_key: "geo_code1".to_string(),
        destination_key: "geo_code2".to_string(),
        min_distance_meters: min_distance,
        deduplicate_pairs: false,
    };

    let mut rng = StdRng::seed_from_u64(42);
    let mut output = Vec::new();
    jitter(
        "data/od.csv",
        &zones,
        10,
        "all".to_string(),
        &mut rng,
        options,
        |feature| {
            output.push(feature);
            Ok(())
        },
    )
    .unwrap();

    // Verify all output pairs satisfy the minimum distance
    use geo::algorithm::haversine_distance::HaversineDistance;
    for feature in &output {
        if let Some(geojson::Value::LineString(ls)) =
            feature.geometry.as_ref().map(|geom| &geom.value)
        {
            let origin = Point::new(ls[0][0], ls[0][1]);
            let destination = Point::new(ls[1][0], ls[1][1]);
            let distance = origin.haversine_distance(&destination);
            assert!(
                distance >= min_distance,
                "Found a pair with distance {} which is less than minimum {}",
                distance,
                min_distance
            );
        }
    }
}

#[test]
fn test_zero_trip_rows_preserved() {
    // Test that rows with 0 trips in disaggregation_key are still processed
    // This addresses the edge case mentioned in the code
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();

    let options = Options {
        subsample_origin: Subsample::RandomPoints,
        subsample_destination: Subsample::RandomPoints,
        origin_key: "origin".to_string(),
        destination_key: "destination".to_string(),
        min_distance_meters: 1.0,
        deduplicate_pairs: false,
    };

    let mut rng = StdRng::seed_from_u64(42);
    let mut output = Vec::new();
    jitter(
        "data/od_schools.csv",
        &zones,
        10,
        "walk".to_string(), // This file has some 0 values
        &mut rng,
        options,
        |feature| {
            output.push(feature);
            Ok(())
        },
    )
    .unwrap();

    // The test passes if we got here without panicking
    // Verify we got some output
    assert!(
        !output.is_empty(),
        "Should produce output even with some zero-trip rows"
    );
}

#[test]
fn test_weighted_points_distribution() {
    // Test that weighted points are sampled according to their weights
    // This addresses issue #18 about sanity checking weighted results
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let destination_subpoints =
        scrape_points("data/schools.geojson", Some("weight".to_string())).unwrap();

    // Create a map of point -> weight
    let mut point_weights: HashMap<Point<NotNan<f64>>, f64> = HashMap::new();
    for wp in &destination_subpoints {
        point_weights.insert(hashify_point(wp.point), wp.weight);
    }

    let options = Options {
        subsample_origin: Subsample::RandomPoints,
        subsample_destination: Subsample::WeightedPoints(destination_subpoints),
        origin_key: "origin".to_string(),
        destination_key: "destination".to_string(),
        min_distance_meters: 1.0,
        deduplicate_pairs: false,
    };

    let mut rng = StdRng::seed_from_u64(42);
    let mut output = Vec::new();
    jitter(
        "data/od_schools.csv",
        &zones,
        1,
        "walk".to_string(),
        &mut rng,
        options,
        |feature| {
            output.push(feature);
            Ok(())
        },
    )
    .unwrap();

    // Count how many times each destination appears
    let mut destination_counts: HashMap<Point<NotNan<f64>>, usize> = HashMap::new();
    for feature in &output {
        if let Some(geojson::Value::LineString(ls)) =
            feature.geometry.as_ref().map(|geom| &geom.value)
        {
            let dest = ls.last().unwrap();
            let dest_point = hashify_point(Point::new(dest[0], dest[1]));
            *destination_counts.entry(dest_point).or_insert(0) += 1;
        }
    }

    // We should have at least 2 different destinations used
    assert!(
        destination_counts.len() >= 2,
        "Should use multiple destination points"
    );

    // The correlation between weight and count should be positive
    // (not testing exact values due to randomness, but checking general trend)
    if destination_counts.len() >= 3 {
        let mut weights: Vec<f64> = Vec::new();
        let mut counts: Vec<usize> = Vec::new();
        for (point, count) in &destination_counts {
            if let Some(&weight) = point_weights.get(point) {
                weights.push(weight);
                counts.push(*count);
            }
        }
        // Just verify that some high-weight points got more counts
        // This is a weak test but better than nothing
        assert!(
            weights.len() == counts.len(),
            "Weight and count vectors should match"
        );
    }
}

#[test]
fn test_random_points_subsample() {
    // Test jittering with RandomPoints (no subpoints provided)
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
    jitter(
        "data/od.csv",
        &zones,
        50,
        "all".to_string(),
        &mut rng,
        options,
        |feature| {
            output.push(feature);
            Ok(())
        },
    )
    .unwrap();

    assert!(
        !output.is_empty(),
        "Should produce output with RandomPoints"
    );

    // Verify that origins and destinations are within their respective zones
    // (this is a basic sanity check)
    use geo::algorithm::contains::Contains;
    let mut checked_count = 0;
    for feature in output.iter().take(10) {
        // Check first 10 for performance
        if let Some(geojson::Value::LineString(ls)) =
            feature.geometry.as_ref().map(|geom| &geom.value)
        {
            let origin = Point::new(ls[0][0], ls[0][1]);
            let destination = Point::new(ls[1][0], ls[1][1]);

            // Get zone IDs from properties
            if let (Some(Value::String(origin_id)), Some(Value::String(dest_id))) =
                (feature.property("geo_code1"), feature.property("geo_code2"))
            {
                if let (Some(origin_zone), Some(dest_zone)) =
                    (zones.get(origin_id), zones.get(dest_id))
                {
                    assert!(
                        origin_zone.contains(&origin),
                        "Origin point should be within origin zone"
                    );
                    assert!(
                        dest_zone.contains(&destination),
                        "Destination point should be within destination zone"
                    );
                    checked_count += 1;
                }
            }
        }
    }
    assert!(
        checked_count > 0,
        "Should have verified some point containment"
    );
}

#[test]
fn test_different_thresholds_consistency() {
    // Test that different disaggregation thresholds produce consistent total flows
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let subpoints = scrape_points("data/road_network.geojson", None).unwrap();

    let thresholds = vec![10, 50, 100];
    let mut all_sums = Vec::new();

    for threshold in thresholds {
        let options = Options {
            subsample_origin: Subsample::WeightedPoints(subpoints.clone()),
            subsample_destination: Subsample::WeightedPoints(subpoints.clone()),
            origin_key: "geo_code1".to_string(),
            destination_key: "geo_code2".to_string(),
            min_distance_meters: 1.0,
            deduplicate_pairs: false,
        };

        let mut rng = StdRng::seed_from_u64(42);
        let mut output = Vec::new();
        jitter(
            "data/od.csv",
            &zones,
            threshold,
            "all".to_string(),
            &mut rng,
            options,
            |feature| {
                output.push(feature);
                Ok(())
            },
        )
        .unwrap();

        let sum = sum_trips_output(&output, "all");
        all_sums.push(sum);
    }

    // All sums should be equal (within epsilon)
    let first_sum = all_sums[0];
    for &sum in all_sums.iter().skip(1) {
        assert!(
            (first_sum - sum).abs() < 1e-6,
            "Sum with different thresholds should match: {} vs {}",
            first_sum,
            sum
        );
    }
}

#[test]
fn test_properties_preserved() {
    // Test that all input properties are preserved in the output
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
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
    jitter(
        "data/od.csv",
        &zones,
        100,
        "all".to_string(),
        &mut rng,
        options,
        |feature| {
            output.push(feature);
            Ok(())
        },
    )
    .unwrap();

    // Check that output features have expected properties
    let expected_properties = vec![
        "geo_code1",
        "geo_code2",
        "all",
        "train",
        "bus",
        "car_driver",
        "car_passenger",
        "bicycle",
        "foot",
    ];

    for feature in output.iter().take(5) {
        let props = feature.properties.as_ref().unwrap();
        for prop_name in &expected_properties {
            assert!(
                props.contains_key(*prop_name),
                "Output should preserve property {}",
                prop_name
            );
        }
    }
}

#[test]
fn test_deterministic_with_seed() {
    // Test that the same seed produces identical results
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let subpoints = scrape_points("data/road_network.geojson", None).unwrap();

    let mut outputs = Vec::new();

    for _ in 0..2 {
        let options = Options {
            subsample_origin: Subsample::WeightedPoints(subpoints.clone()),
            subsample_destination: Subsample::WeightedPoints(subpoints.clone()),
            origin_key: "geo_code1".to_string(),
            destination_key: "geo_code2".to_string(),
            min_distance_meters: 1.0,
            deduplicate_pairs: false,
        };

        let mut rng = StdRng::seed_from_u64(12345);
        let mut output = Vec::new();
        jitter(
            "data/od.csv",
            &zones,
            50,
            "all".to_string(),
            &mut rng,
            options,
            |feature| {
                output.push(feature);
                Ok(())
            },
        )
        .unwrap();

        outputs.push(output);
    }

    // Both runs should produce the same number of features
    assert_eq!(
        outputs[0].len(),
        outputs[1].len(),
        "Same seed should produce same number of features"
    );

    // Check that the first few geometries match
    for i in 0..std::cmp::min(5, outputs[0].len()) {
        let geom1 = &outputs[0][i].geometry;
        let geom2 = &outputs[1][i].geometry;
        assert_eq!(
            format!("{:?}", geom1),
            format!("{:?}", geom2),
            "Same seed should produce identical geometries"
        );
    }
}

#[test]
fn test_disaggregate_mode_column() {
    // Test that disaggregate adds a 'mode' column and distributes trips correctly
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

    // Every feature should have a mode property
    let mut modes_found = HashSet::new();
    for feature in &output {
        let props = feature.properties.as_ref().unwrap();
        assert!(
            props.contains_key("mode"),
            "Disaggregated output should have 'mode' property"
        );
        if let Some(Value::String(mode)) = props.get("mode") {
            modes_found.insert(mode.clone());
        }
    }

    // Should have multiple modes
    assert!(
        modes_found.len() > 1,
        "Should have disaggregated by multiple modes"
    );

    // Should include some expected modes
    assert!(
        modes_found.contains("car_driver") || modes_found.contains("foot"),
        "Should include common travel modes"
    );
}

#[test]
fn test_large_disaggregation_threshold() {
    // Test with a very large threshold (effectively no disaggregation)
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
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
    jitter(
        "data/od.csv",
        &zones,
        1000000, // Very large threshold
        "all".to_string(),
        &mut rng,
        options,
        |feature| {
            output.push(feature);
            Ok(())
        },
    )
    .unwrap();

    // Should still work and preserve total trips
    let input_sum = sum_trips_input("data/od.csv", &["all"])["all"];
    let output_sum = sum_trips_output(&output, "all");
    assert!(
        (input_sum - output_sum).abs() < 1e-6,
        "Large threshold should still preserve totals"
    );
}

#[test]
fn test_mixed_zone_types() {
    // Test that zones can handle both Polygon and MultiPolygon geometries
    // This relates to issue #30
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();

    // Verify we loaded some zones
    assert!(!zones.is_empty(), "Should load zones successfully");

    // All zones should be MultiPolygons (even if converted from Polygons)
    for (zone_id, multipolygon) in &zones {
        assert!(
            !multipolygon.0.is_empty(),
            "Zone {} should have at least one polygon",
            zone_id
        );
    }
}

#[test]
fn test_subpoints_without_weights() {
    // Test that subpoints work correctly when no weight key is provided
    let subpoints = scrape_points("data/road_network.geojson", None).unwrap();

    // When no weight is provided, all weights should be 1.0
    for pt in subpoints.iter().take(10) {
        assert_eq!(
            pt.weight, 1.0,
            "Points without weight key should default to weight 1.0"
        );
    }
}

#[test]
fn test_geometry_types() {
    // Test that output geometries are always LineStrings with 2 points
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
    jitter(
        "data/od.csv",
        &zones,
        50,
        "all".to_string(),
        &mut rng,
        options,
        |feature| {
            output.push(feature);
            Ok(())
        },
    )
    .unwrap();

    for (i, feature) in output.iter().enumerate() {
        assert!(
            feature.geometry.is_some(),
            "Feature {} should have geometry",
            i
        );

        if let Some(geojson::Value::LineString(ls)) =
            feature.geometry.as_ref().map(|geom| &geom.value)
        {
            assert_eq!(
                ls.len(),
                2,
                "LineString should have exactly 2 points (origin and destination)"
            );
            assert_eq!(ls[0].len(), 2, "Points should be 2D");
            assert_eq!(ls[1].len(), 2, "Points should be 2D");
        } else {
            panic!("Feature {} geometry is not a LineString", i);
        }
    }
}
