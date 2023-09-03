use std::io::BufReader;

use anyhow::{bail, Result};
use fs_err::File;
use geo::CoordsIter;
use geo_types::Geometry;
use geojson::FeatureReader;

use crate::WeightedPoint;

/// Extract all points from a GeoJSON file. If `weight_key` is specified, use this numeric property
/// per feature as a relative weight for the point. If unspecified, every point will be equally
/// weighted.
///
/// TODO: Note that the returned points are not deduplicated.
pub fn scrape_points(path: &str, weight_key: Option<String>) -> Result<Vec<WeightedPoint>> {
    let reader = FeatureReader::from_reader(BufReader::new(File::open(path)?));
    let mut points = Vec::new();
    for feature in reader.features() {
        let feature = feature?;
        let weight = if let Some(ref key) = weight_key {
            if let Some(weight) = feature.property(key).and_then(|x| x.as_f64()) {
                weight
            } else {
                bail!("Feature doesn't have a numeric {} key: {:?}", key, feature);
            }
        } else {
            1.0
        };
        if let Some(geom) = feature.geometry {
            let geom: Geometry<f64> = geom.try_into()?;
            for pt in geom.coords_iter() {
                points.push(WeightedPoint {
                    point: pt.into(),
                    weight,
                });
            }
        }
    }
    Ok(points)
}
