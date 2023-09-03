use std::io::BufWriter;

use anyhow::Result;
use clap::Parser;
use fs_err::File;
use geojson::FeatureWriter;
use rand::rngs::StdRng;
use rand::SeedableRng;

#[derive(Parser)]
#[clap(about, version, author)]
struct Args {
    #[clap(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand)]
enum Action {
    /// Import raw data and build an activity model for a region
    Jitter {
        #[clap(flatten)]
        common: CommonArgs,

        /// What's the maximum number of trips per output OD row that's allowed? If an input OD row
        /// contains less than this, it will appear in the output without transformation. Otherwise,
        /// the input row is repeated until the sum matches the original value, but each output row
        /// obeys this maximum.
        #[clap(long)]
        disaggregation_threshold: usize,

        /// Which column in the OD row specifies the total number of trips to disaggregate?
        #[clap(long, default_value = "all")]
        disaggregation_key: String,
    },
    /// Fully disaggregate input desire lines into output representing one trip each, with a `mode`
    /// column.
    Disaggregate {
        #[clap(flatten)]
        common: CommonArgs,
    },
}

#[derive(Clone, Parser)]
struct CommonArgs {
    /// The path to a CSV file with aggregated origin/destination data
    #[clap(long)]
    od_csv_path: String,

    /// The path to a GeoJSON file with named zones
    #[clap(long)]
    zones_path: String,

    /// The path to a file where the output will be written
    #[clap(long)]
    output_path: String,

    /// Output a FlatGeobuf file (without an index) if true, or a GeoJSON file by default
    #[clap(long)]
    output_fgb: bool,

    /// The path to a GeoJSON file to use for sampling subpoints for origin zones. If this isn't
    /// specified, random points within each zone will be used instead.
    #[clap(long)]
    subpoints_origins_path: Option<String>,
    /// If specified, this column will be used to more frequently choose subpoints in
    /// `subpoints_origins_path` with a higher weight value. Otherwise all subpoints will be
    /// equally likely to be chosen.
    #[clap(long)]
    weight_key_origins: Option<String>,

    /// The path to a GeoJSON file to use for sampling subpoints for destination zones. If this
    /// isn't specified, random points within each zone will be used instead.
    #[clap(long)]
    subpoints_destinations_path: Option<String>,
    /// If specified, this column will be used to more frequently choose subpoints in
    /// `subpoints_destinations_path` with a higher weight value. Otherwise all subpoints will be
    /// equally likely to be chosen.
    #[clap(long)]
    weight_key_destinations: Option<String>,

    /// In the zones GeoJSON file, which property is the name of a zone
    #[clap(long, default_value = "InterZone")]
    zone_name_key: String,
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
    /// Guarantee that jittered origin and destination points are at least this distance apart.
    #[clap(long, default_value = "1.0")]
    min_distance_meters: f64,
    /// Prevent duplicate (origin, destination) pairs from appearing in the output. This may
    /// increase memory and runtime requirements. Note the duplication uses the floating point
    /// precision of the input data, and only consider geometry (not any properties).
    #[clap(long)]
    deduplicate_pairs: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    // TODO Remove the clone
    let common = match args.action {
        Action::Jitter { ref common, .. } => common.clone(),
        Action::Disaggregate { ref common, .. } => common.clone(),
    };
    let output_path = common.output_path.clone();

    if common.output_fgb {
        let mut fgb = flatgeobuf::FgbWriter::create_with_options(
            "odjitter",
            flatgeobuf::GeometryType::LineString,
            flatgeobuf::FgbWriterOptions {
                write_index: false,
                ..Default::default()
            },
        )?;
        let write_feature = |feature| {
            // TODO Is there a cheaper way to make a GeozeroDatasource, or something else we should
            // generate from the API?
            fgb.add_feature(geozero::geojson::GeoJson(&serde_json::to_string(&feature)?))?;
            Ok(())
        };
        run(args, common, write_feature)?;
        println!("Writing {output_path}");
        let mut file = std::io::BufWriter::new(File::create(&output_path)?);
        fgb.write(&mut file)?;
    } else {
        // Write GeoJSON to a file. Instead of collecting the whole FeatureCollection in memory, write
        // each feature as we get it.
        let mut writer = FeatureWriter::from_writer(BufWriter::new(File::create(&output_path)?));
        let write_feature = |feature| {
            writer.write_feature(&feature)?;
            Ok(())
        };

        run(args, common, write_feature)?;
    }

    println!("Wrote {output_path}");
    Ok(())
}

fn run<F: FnMut(geojson::Feature) -> Result<()>>(
    args: Args,
    common: CommonArgs,
    write_feature: F,
) -> Result<()> {
    let zones = odjitter::load_zones(&common.zones_path, &common.zone_name_key)?;
    println!("Scraped {} zones from {}", zones.len(), common.zones_path);

    let subsample_origin = if let Some(ref path) = common.subpoints_origins_path {
        let subpoints = odjitter::scrape_points(path, common.weight_key_origins)?;
        println!("Scraped {} subpoints from {}", subpoints.len(), path);
        odjitter::Subsample::WeightedPoints(subpoints)
    } else {
        odjitter::Subsample::RandomPoints
    };
    let subsample_destination = if let Some(ref path) = common.subpoints_destinations_path {
        let subpoints = odjitter::scrape_points(path, common.weight_key_destinations)?;
        println!("Scraped {} subpoints from {}", subpoints.len(), path);
        odjitter::Subsample::WeightedPoints(subpoints)
    } else {
        odjitter::Subsample::RandomPoints
    };

    let options = odjitter::Options {
        subsample_origin,
        subsample_destination,
        origin_key: common.origin_key,
        destination_key: common.destination_key,
        min_distance_meters: common.min_distance_meters,
        deduplicate_pairs: common.deduplicate_pairs,
    };
    let mut rng = if let Some(seed) = common.rng_seed {
        StdRng::seed_from_u64(seed)
    } else {
        StdRng::from_entropy()
    };

    match args.action {
        Action::Jitter {
            disaggregation_threshold,
            disaggregation_key,
            ..
        } => {
            odjitter::jitter(
                common.od_csv_path,
                &zones,
                disaggregation_threshold,
                disaggregation_key,
                &mut rng,
                options,
                write_feature,
            )?;
        }
        Action::Disaggregate { .. } => {
            odjitter::disaggregate(common.od_csv_path, &zones, &mut rng, options, write_feature)?;
        }
    }
    Ok(())
}
