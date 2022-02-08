use anyhow::Result;
use geo::coords_iter::CoordsIter;
use geo_types::{Geometry, Point};
use geojson::GeoJson;

/// Extract all points from a GeoJSON file.
///
/// TODO: Note that the returned points are not deduplicated.
pub fn scrape_points(path: &str) -> Result<Vec<Point<f64>>> {
    let geojson_input = fs_err::read_to_string(path)?;
    let geojson = geojson_input.parse::<GeoJson>()?;
    let mut points = Vec::new();
    if let GeoJson::FeatureCollection(collection) = geojson {
        for feature in collection.features {
            if let Some(geom) = feature.geometry {
                let geom: Geometry<f64> = geom.try_into()?;
                for pt in geom.coords_iter() {
                    points.push(pt.into());
                }
            }
        }
    }
    Ok(points)
}
