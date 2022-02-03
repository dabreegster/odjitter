use anyhow::Result;
use geo_types::{Point, Polygon};
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
                points.extend(geometry_to_points(geom.try_into().unwrap()));
            }
        }
    }
    Ok(points)
}

fn geometry_to_points(geom: geo_types::Geometry<f64>) -> Vec<Point<f64>> {
    let mut points = Vec::new();
    // We can't use MapCoordsInplace
    match geom {
        geo_types::Geometry::Point(pt) => {
            points.push(pt);
        }
        geo_types::Geometry::Line(line) => {
            let (a, b) = line.points();
            points.push(a);
            points.push(b);
        }
        geo_types::Geometry::LineString(ls) => {
            points.extend(ls.into_points());
        }
        geo_types::Geometry::Polygon(poly) => {
            points.extend(polygon_to_points(poly));
        }
        geo_types::Geometry::MultiPoint(pts) => {
            points.extend(pts);
        }
        geo_types::Geometry::MultiLineString(list) => {
            for ls in list {
                points.extend(ls.into_points());
            }
        }
        geo_types::Geometry::MultiPolygon(list) => {
            for poly in list {
                points.extend(polygon_to_points(poly));
            }
        }
        geo_types::Geometry::GeometryCollection(list) => {
            for geom in list {
                points.extend(geometry_to_points(geom));
            }
        }
        geo_types::Geometry::Rect(rect) => {
            points.extend(polygon_to_points(rect.to_polygon()));
        }
        geo_types::Geometry::Triangle(tri) => {
            points.extend(polygon_to_points(tri.to_polygon()));
        }
    }
    points
}

fn polygon_to_points(poly: Polygon<f64>) -> Vec<Point<f64>> {
    let mut points = Vec::new();
    let (exterior, interiors) = poly.into_inner();
    points.extend(exterior.into_points());
    for ls in interiors {
        points.extend(ls.into_points());
    }
    points
}
