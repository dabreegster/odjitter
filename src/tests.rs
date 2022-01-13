use geojson::GeoJson;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::{Map, Value};

use crate::{jitter, load_zones, scrape_points, Options, Subsample};

#[test]
fn test_sums_match() {
    let zones = load_zones("data/zones.geojson", "InterZone").unwrap();
    let input_sum = sum_trips_input("data/od.csv");

    for max_per_od in [1, 10, 100, 1000] {
        let subpoints = scrape_points("data/road_network.geojson").unwrap();
        let options = Options {
            max_per_od,
            subsample: Subsample::UnweightedPoints(subpoints),
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

        let output_sum = sum_trips_output(&output);
        let epsilon = 1e-6;
        assert!(
            (input_sum - output_sum).abs() < epsilon,
            "Number of trips in input {} and jittered output {} don't match for max_per_od = {}",
            input_sum,
            output_sum,
            max_per_od
        );

        // TODO Test that sums match for each mode
    }
}

// TODO Test zone names that look numeric and contain leading 0's

fn sum_trips_input(csv_path: &str) -> f64 {
    let mut total = 0.0;
    for rec in csv::Reader::from_path(csv_path).unwrap().deserialize() {
        let map: Map<String, Value> = rec.unwrap();
        if let Value::Number(x) = &map["all"] {
            total += x.as_f64().unwrap();
        }
    }
    total
}

fn sum_trips_output(gj: &GeoJson) -> f64 {
    let mut total = 0.0;
    if let GeoJson::FeatureCollection(fc) = gj {
        for feature in &fc.features {
            total += feature.property("all").unwrap().as_f64().unwrap();
        }
    }
    total
}
