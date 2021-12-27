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

    let max_per_od = 10;
    let gj = odjitter::jitter(
        &zones,
        "data/od.csv",
        max_per_od,
        &mut StdRng::seed_from_u64(42),
        Some(all_subpoints),
    )?;

    let mut file = File::create("output.geojson")?;
    write!(file, "{}", serde_json::to_string_pretty(&gj)?)?;
    println!("Wrote output.geojson");

    Ok(())
}
