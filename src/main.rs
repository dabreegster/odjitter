use anyhow::Result;
use clap::Parser;
use fs_err::File;
use rand::rngs::StdRng;
use rand::SeedableRng;

#[derive(Parser)]
#[clap(about, version, author)]
struct Args {
    /// The path to a CSV file with aggregated origin/destination data
    #[clap(long)]
    od_csv_path: String,

    /// The path to a GeoJSON file with named zones
    #[clap(long)]
    zones_path: String,

    /// The path to a GeoJSON file where the disaggregated output will be written
    #[clap(long)]
    output_path: String,

    /// The path to a GeoJSON file with subpoints to sample from. If this isn't specified, random
    /// points within each zone will be used instead.
    #[clap(long)]
    subpoints_path: Option<String>,

    /// What's the maximum number of trips per output OD row that's allowed? If an input OD row
    /// contains less than this, it will appear in the output without transformation. Otherwise,
    /// the input row is repeated until the sum matches the original value, but each output row
    /// obeys this maximum.
    #[clap(long)]
    max_per_od: usize,

    /// In the zones GeoJSON file, which property is the name of a zone
    #[clap(long, default_value = "InterZone")]
    zone_name_key: String,
    /// Which column in the OD row specifies the total number of trips to disaggregate?
    #[clap(long, default_value = "all")]
    all_key: String,
    /// Which column in the OD row specifies the zone where trips originate?
    #[clap(long, default_value = "geo_code1")]
    origin_key: String,
    /// Which column in the OD row specifies the zone where trips ends?
    #[clap(long, default_value = "geo_code2")]
    destination_key: String,
    /// By default, the output will be different every time the tool is run, based on a different
    /// random number generator seed. Specify this to get deterministic behavior, given the same
    /// input.
    #[clap(long)]
    rng_seed: Option<u64>,
    /// Guarantee that jittered points are at least this distance apart.
    #[clap(long, default_value = "1.0")]
    min_distance_meters: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let zones = odjitter::load_zones(&args.zones_path, &args.zone_name_key)?;
    println!("Scraped {} zones from {}", zones.len(), args.zones_path);

    let subsample = if let Some(ref path) = args.subpoints_path {
        let subpoints = odjitter::scrape_points(path)?;
        println!("Scraped {} subpoints from {}", subpoints.len(), path);
        odjitter::Subsample::UnweightedPoints(subpoints)
    } else {
        odjitter::Subsample::RandomPoints
    };

    let options = odjitter::Options {
        max_per_od: args.max_per_od,
        subsample,
        all_key: args.all_key,
        origin_key: args.origin_key,
        destination_key: args.destination_key,
        min_distance_meters: args.min_distance_meters,
    };
    let mut rng = if let Some(seed) = args.rng_seed {
        StdRng::seed_from_u64(seed)
    } else {
        StdRng::from_entropy()
    };

    let mut file = std::io::BufWriter::new(File::create(&args.output_path)?);
    odjitter::jitter(args.od_csv_path, &zones, &mut rng, options, &mut file)?;
    println!("Wrote {}", args.output_path);

    Ok(())
}
