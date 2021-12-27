use std::fs::File;
use std::io::Write;

use anyhow::Result;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn main() -> Result<()> {
    let zones = odjitter::load_zones("data/zones.geojson", "InterZone")?;
    println!("Scraped {} zones", zones.len());

    let all_subpoints = odjitter::scrape_points("data/road_network.geojson")?;
    println!("Scraped {} subpoints", all_subpoints.len());

    let options = odjitter::Options {
        max_per_od: 10,
        subsample: odjitter::Subsample::UnweightedPoints(all_subpoints),
        all_key: "all".to_string(),
        origin_key: "geo_code1".to_string(),
        destination_key: "geo_code2".to_string(),
    };
    let gj = odjitter::jitter(
        "data/od.csv",
        &zones,
        &mut StdRng::seed_from_u64(42),
        options,
    )?;

    let mut file = File::create("output.geojson")?;
    write!(file, "{}", serde_json::to_string_pretty(&gj)?)?;
    println!("Wrote output.geojson");

    Ok(())
}
