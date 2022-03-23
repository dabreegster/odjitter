use anyhow::{bail, Result};
use geo::coords_iter::CoordsIter;
use geo_types::Geometry;
use geojson::GeoJson;

use crate::WeightedPoint;

/// Extract all points from a GeoJSON file. If `weight_key` is specified, use this numeric property
/// per feature as a relative weight for the point. If unspecified, every point will be equally
/// weighted.
///
/// TODO: Note that the returned points are not deduplicated.
pub fn scrape_points(path: &str, weight_key: Option<String>) -> Result<Vec<WeightedPoint>> {
    let geojson_input = fs_err::read_to_string(path)?;
    let geojson = geojson_input.parse::<GeoJson>()?;
    let mut points = Vec::new();
    if let GeoJson::FeatureCollection(collection) = geojson {
        for feature in collection.features {
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
    }
    Ok(points)
}
