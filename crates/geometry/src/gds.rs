use std::collections::{BTreeMap, BTreeSet};

use gds21::{GdsElement, GdsLibrary};
use serde::{Deserialize, Serialize};

type GdsPoint = [f64; 2];
type GdsContour = Vec<GdsPoint>;
type GdsShape = Vec<GdsContour>;
type GdsShapes = Vec<GdsShape>;

const ROUND_CAP_SEGMENTS: usize = 12;

#[derive(Debug, Deserialize)]
pub(crate) struct GdsLayoutRequest {
    pub(crate) cell: String,
    pub(crate) layers: BTreeMap<String, [i16; 2]>,
    #[serde(default, rename = "path-tolerance")]
    pub(crate) path_tolerance: f64,
}

#[derive(Debug, PartialEq, Serialize)]
pub(crate) struct GdsLayout {
    pub(crate) origin: GdsPoint,
    pub(crate) size: GdsPoint,
    pub(crate) layers: BTreeMap<String, GdsShapes>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct GdsInfo {
    pub(crate) library: String,
    pub(crate) db_unit_meters: f64,
    pub(crate) user_unit_meters: f64,
    pub(crate) cells: Vec<String>,
    pub(crate) layers: Vec<[i16; 2]>,
}

pub(crate) fn inspect(bytes: &[u8]) -> Result<GdsInfo, String> {
    let library = GdsLibrary::from_bytes(bytes.to_vec())
        .map_err(|error| format!("could not parse GDS: {error}"))?;

    let mut cells = library
        .structs
        .iter()
        .map(|cell| cell.name.clone())
        .collect::<Vec<_>>();
    cells.sort();

    let layers = library
        .structs
        .iter()
        .flat_map(|cell| &cell.elems)
        .filter_map(|element| match element {
            GdsElement::GdsBoundary(boundary) => Some([boundary.layer, boundary.datatype]),
            GdsElement::GdsPath(path) => Some([path.layer, path.datatype]),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    Ok(GdsInfo {
        library: library.name,
        db_unit_meters: library.units.db_unit(),
        // gds21 0.2.0 returns user-units per metre here, despite the method name
        // and documentation. Invert it to recover the user-unit size in metres.
        user_unit_meters: 1.0 / library.units.user_unit(),
        cells,
        layers,
    })
}

pub(crate) fn extract(bytes: &[u8], request: GdsLayoutRequest) -> Result<GdsLayout, String> {
    let library = GdsLibrary::from_bytes(bytes.to_vec())
        .map_err(|error| format!("could not parse GDS: {error}"))?;
    let cell = library
        .structs
        .iter()
        .find(|cell| cell.name == request.cell)
        .ok_or_else(|| format!("GDS cell {:?} does not exist", request.cell))?;

    if cell.elems.iter().any(|element| {
        matches!(
            element,
            GdsElement::GdsStructRef(_) | GdsElement::GdsArrayRef(_)
        )
    }) {
        return Err(format!(
            "GDS cell {:?} contains references; hierarchy is not supported yet",
            request.cell
        ));
    }
    if !request.path_tolerance.is_finite() || request.path_tolerance < 0.0 {
        return Err(format!(
            "GDS path tolerance must be a finite non-negative number; got {}",
            request.path_tolerance,
        ));
    }

    let mut layers = request
        .layers
        .keys()
        .map(|name| (name.clone(), GdsShapes::new()))
        .collect::<BTreeMap<_, _>>();
    let mut bounds: Option<[f64; 4]> = None;
    let nanometers_per_database_unit = library.units.db_unit() / 1e-9;
    let path_tolerance = request.path_tolerance / nanometers_per_database_unit;

    for element in &cell.elems {
        let (layer, datatype) = match element {
            GdsElement::GdsBoundary(boundary) => (boundary.layer, boundary.datatype),
            GdsElement::GdsPath(path) => (path.layer, path.datatype),
            _ => continue,
        };
        let Some((name, _)) = request
            .layers
            .iter()
            .find(|(_, requested)| **requested == [layer, datatype])
        else {
            continue;
        };
        let shapes = match element {
            GdsElement::GdsBoundary(boundary) => {
                let mut contour = boundary
                    .xy
                    .iter()
                    .map(|point| [point.x as f64, point.y as f64])
                    .collect::<GdsContour>();
                if contour.len() >= 2 && contour.first() == contour.last() {
                    contour.pop();
                }
                vec![vec![contour]]
            }
            GdsElement::GdsPath(path) => path_to_shapes(path, path_tolerance)?,
            _ => unreachable!("element was checked above"),
        };

        for shape in shapes {
            if shape.first().is_some_and(|contour| contour.len() < 3) {
                continue;
            }
            for contour in &shape {
                update_bounds(&mut bounds, contour);
            }
            layers.get_mut(name).unwrap().push(shape);
        }
    }

    let [min_x, min_y, max_x, max_y] =
        bounds.ok_or_else(|| "selected GDS layers contain no boundary polygons".to_string())?;
    let origin = [
        min_x * nanometers_per_database_unit,
        min_y * nanometers_per_database_unit,
    ];

    for shapes in layers.values_mut() {
        for shape in shapes {
            for contour in shape {
                for point in contour {
                    point[0] = point[0] * nanometers_per_database_unit - origin[0];
                    point[1] = point[1] * nanometers_per_database_unit - origin[1];
                }
            }
        }
    }

    Ok(GdsLayout {
        origin,
        size: [
            (max_x - min_x) * nanometers_per_database_unit,
            (max_y - min_y) * nanometers_per_database_unit,
        ],
        layers,
    })
}

fn update_bounds(bounds: &mut Option<[f64; 4]>, contour: &GdsContour) {
    for &[x, y] in contour {
        *bounds = Some(match *bounds {
            Some([min_x, min_y, max_x, max_y]) => {
                [min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y)]
            }
            None => [x, y, x, y],
        });
    }
}

fn path_to_shapes(path: &gds21::GdsPath, tolerance: f64) -> Result<GdsShapes, String> {
    let width = path.width.ok_or_else(|| {
        format!(
            "GDS path on layer {}/{} has no width and cannot become an area mask",
            path.layer, path.datatype,
        )
    })?;
    if width <= 0 {
        return Err(format!(
            "GDS path on layer {}/{} has non-positive width {width}",
            path.layer, path.datatype,
        ));
    }

    let points =
        deduplicate_path_points(path.xy.iter().map(|point| [point.x as f64, point.y as f64]));
    if points.len() < 2 {
        return Err(format!(
            "GDS path on layer {}/{} needs at least two distinct points",
            path.layer, path.datatype,
        ));
    }
    let mut points = simplify_polyline(points, tolerance);

    let half_width = width as f64 / 2.0;
    let (start_extension, end_extension, round_ends) = match path.path_type.unwrap_or(0) {
        0 => (0.0, 0.0, false),
        1 => (0.0, 0.0, true),
        2 => (half_width, half_width, false),
        4 => (
            path.begin_extn.unwrap_or(0) as f64,
            path.end_extn.unwrap_or(0) as f64,
            false,
        ),
        path_type => {
            return Err(format!(
                "GDS path on layer {}/{} has unsupported path type {path_type}",
                path.layer, path.datatype,
            ));
        }
    };

    let start_direction = unit_direction(points[0], points[1]);
    let last_index = points.len() - 1;
    let end_direction = unit_direction(points[last_index - 1], points[last_index]);
    points[0] = add(points[0], scale(start_direction, -start_extension));
    points[last_index] = add(points[last_index], scale(end_direction, end_extension));

    let left = offset_rail(&points, half_width).map_err(|reason| {
        format!(
            "could not offset GDS path on layer {}/{}: {reason}",
            path.layer, path.datatype,
        )
    })?;
    let right = offset_rail(&points, -half_width).map_err(|reason| {
        format!(
            "could not offset GDS path on layer {}/{}: {reason}",
            path.layer, path.datatype,
        )
    })?;

    let mut contour = Vec::with_capacity(
        left.len() + right.len() + usize::from(round_ends) * 2 * (ROUND_CAP_SEGMENTS - 1),
    );
    contour.extend(left);
    if round_ends {
        append_round_cap(
            &mut contour,
            points[last_index],
            end_direction,
            half_width,
            false,
        );
    }
    contour.extend(right.into_iter().rev());
    if round_ends {
        append_round_cap(&mut contour, points[0], start_direction, half_width, true);
    }

    Ok(vec![vec![contour]])
}

fn deduplicate_path_points(points: impl IntoIterator<Item = GdsPoint>) -> Vec<GdsPoint> {
    let mut result = Vec::new();
    for point in points {
        if result.last().copied() != Some(point) {
            result.push(point);
        }
    }
    result
}

fn unit_direction(start: GdsPoint, end: GdsPoint) -> GdsPoint {
    let [x, y] = subtract(end, start);
    let length = x.hypot(y);
    debug_assert!(length > 0.0);
    [x / length, y / length]
}

fn offset_rail(points: &[GdsPoint], distance: f64) -> Result<Vec<GdsPoint>, &'static str> {
    let directions = points
        .windows(2)
        .map(|segment| unit_direction(segment[0], segment[1]))
        .collect::<Vec<_>>();
    let mut rail = Vec::with_capacity(points.len());
    rail.push(offset_point(points[0], directions[0], distance));

    for index in 1..points.len() - 1 {
        let previous = directions[index - 1];
        let next = directions[index];
        let previous_line = (offset_point(points[index], previous, distance), previous);
        let next_line = (offset_point(points[index], next, distance), next);
        let Some(intersection) = line_intersection(previous_line, next_line) else {
            if dot(previous, next) > 0.0 {
                rail.push(offset_point(points[index], next, distance));
                continue;
            }
            return Err("the centreline contains a 180-degree reversal");
        };
        rail.push(intersection);
    }

    rail.push(offset_point(
        *points.last().unwrap(),
        *directions.last().unwrap(),
        distance,
    ));
    Ok(rail)
}

fn simplify_polyline(points: Vec<GdsPoint>, tolerance: f64) -> Vec<GdsPoint> {
    if tolerance <= 0.0 || points.len() <= 2 {
        return points;
    }
    let mut result = Vec::new();
    simplify_polyline_segment(
        &points,
        0,
        points.len() - 1,
        tolerance * tolerance,
        &mut result,
    );
    result.push(*points.last().unwrap());
    result
}

fn simplify_polyline_segment(
    points: &[GdsPoint],
    first: usize,
    last: usize,
    tolerance_squared: f64,
    result: &mut Vec<GdsPoint>,
) {
    let mut farthest = None;
    let mut farthest_distance = tolerance_squared;
    for index in first + 1..last {
        let distance =
            point_to_segment_distance_squared(points[index], points[first], points[last]);
        if distance > farthest_distance {
            farthest = Some(index);
            farthest_distance = distance;
        }
    }
    if let Some(index) = farthest {
        simplify_polyline_segment(points, first, index, tolerance_squared, result);
        simplify_polyline_segment(points, index, last, tolerance_squared, result);
    } else {
        result.push(points[first]);
    }
}

fn point_to_segment_distance_squared(point: GdsPoint, start: GdsPoint, end: GdsPoint) -> f64 {
    let direction = subtract(end, start);
    let length_squared = dot(direction, direction);
    if length_squared == 0.0 {
        return dot(subtract(point, start), subtract(point, start));
    }
    let projection = (dot(subtract(point, start), direction) / length_squared).clamp(0.0, 1.0);
    let nearest = add(start, scale(direction, projection));
    dot(subtract(point, nearest), subtract(point, nearest))
}

fn offset_point(point: GdsPoint, direction: GdsPoint, distance: f64) -> GdsPoint {
    add(point, [-direction[1] * distance, direction[0] * distance])
}

fn line_intersection(
    first: (GdsPoint, GdsPoint),
    second: (GdsPoint, GdsPoint),
) -> Option<GdsPoint> {
    let denominator = cross(first.1, second.1);
    if denominator.abs() < 1e-12 {
        return None;
    }
    let along_first = cross(subtract(second.0, first.0), second.1) / denominator;
    Some(add(first.0, scale(first.1, along_first)))
}

fn append_round_cap(
    contour: &mut GdsContour,
    center: GdsPoint,
    direction: GdsPoint,
    radius: f64,
    start: bool,
) {
    let direction_angle = direction[1].atan2(direction[0]);
    let first_angle = if start {
        direction_angle - std::f64::consts::FRAC_PI_2
    } else {
        direction_angle + std::f64::consts::FRAC_PI_2
    };
    for step in 1..ROUND_CAP_SEGMENTS {
        let angle = first_angle - std::f64::consts::PI * step as f64 / ROUND_CAP_SEGMENTS as f64;
        contour.push(add(center, [radius * angle.cos(), radius * angle.sin()]));
    }
}

fn cross(left: GdsPoint, right: GdsPoint) -> f64 {
    left[0] * right[1] - left[1] * right[0]
}

fn dot(left: GdsPoint, right: GdsPoint) -> f64 {
    left[0] * right[0] + left[1] * right[1]
}

fn add(left: GdsPoint, right: GdsPoint) -> GdsPoint {
    [left[0] + right[0], left[1] + right[1]]
}

fn subtract(left: GdsPoint, right: GdsPoint) -> GdsPoint {
    [left[0] - right[0], left[1] - right[1]]
}

fn scale(point: GdsPoint, factor: f64) -> GdsPoint {
    [point[0] * factor, point[1] * factor]
}

#[cfg(test)]
mod tests {
    use gds21::{
        GdsBoundary, GdsLibrary, GdsPath, GdsPoint as LibraryPoint, GdsStruct, GdsStructRef,
    };

    use super::*;

    fn boundary(layer: i16, datatype: i16, points: &[(i32, i32)]) -> GdsBoundary {
        GdsBoundary {
            layer,
            datatype,
            xy: LibraryPoint::vec(points),
            ..Default::default()
        }
    }

    fn path(layer: i16, datatype: i16, width: i32, points: &[(i32, i32)]) -> GdsPath {
        GdsPath {
            layer,
            datatype,
            width: Some(width),
            xy: LibraryPoint::vec(points),
            ..Default::default()
        }
    }

    fn shape_bounds(shapes: &GdsShapes) -> [f64; 4] {
        let mut bounds = None;
        for contour in shapes.iter().flatten() {
            update_bounds(&mut bounds, contour);
        }
        bounds.unwrap()
    }

    #[test]
    fn reads_library_cells_units_and_boundary_layers_from_bytes() {
        let mut library = GdsLibrary::new("semi-example");
        let mut top = GdsStruct::new("TOP");
        top.elems.push(
            GdsBoundary {
                layer: 20,
                datatype: 0,
                xy: LibraryPoint::vec(&[(0, 0), (8, 0), (8, 3), (0, 3), (0, 0)]),
                ..Default::default()
            }
            .into(),
        );
        top.elems.push(
            GdsBoundary {
                layer: 1,
                datatype: 0,
                xy: LibraryPoint::vec(&[(1, 1), (7, 1), (7, 2), (1, 2), (1, 1)]),
                ..Default::default()
            }
            .into(),
        );
        library.structs.push(top);

        let mut bytes = Vec::new();
        library.write(&mut bytes).unwrap();

        let info = inspect(&bytes).unwrap();
        assert_eq!(info.library, "semi-example");
        assert_eq!(info.cells, ["TOP"]);
        assert_eq!(info.layers, [[1, 0], [20, 0]]);
        assert_eq!(info.db_unit_meters, 1e-9);
        assert_eq!(info.user_unit_meters, 1e-6);
    }

    #[test]
    fn extracts_named_boundary_layers_in_nanometers() {
        let mut library = GdsLibrary::new("semi-example");
        let mut top = GdsStruct::new("TOP");
        top.elems.push(
            boundary(
                1,
                0,
                &[(100, 200), (110, 200), (110, 206), (100, 206), (100, 200)],
            )
            .into(),
        );
        top.elems.push(
            boundary(
                10,
                2,
                &[(102, 201), (108, 201), (108, 205), (102, 205), (102, 201)],
            )
            .into(),
        );
        library.structs.push(top);

        let mut bytes = Vec::new();
        library.write(&mut bytes).unwrap();
        let layout = extract(
            &bytes,
            GdsLayoutRequest {
                cell: "TOP".into(),
                layers: BTreeMap::from([("active".into(), [1, 0]), ("gate".into(), [10, 2])]),
                path_tolerance: 0.0,
            },
        )
        .unwrap();

        assert_eq!(layout.origin, [100.0, 200.0]);
        assert_eq!(layout.size, [10.0, 6.0]);
        assert_eq!(
            layout.layers["gate"],
            vec![vec![vec![[2.0, 1.0], [8.0, 1.0], [8.0, 5.0], [2.0, 5.0],]]]
        );
    }

    #[test]
    fn extracts_paths_as_width_aware_polygons() {
        let mut library = GdsLibrary::new("semi-example");
        let mut top = GdsStruct::new("TOP");
        top.elems
            .push(path(1, 0, 4, &[(100, 200), (120, 200)]).into());
        library.structs.push(top);

        let mut bytes = Vec::new();
        library.write(&mut bytes).unwrap();
        let layout = extract(
            &bytes,
            GdsLayoutRequest {
                cell: "TOP".into(),
                layers: BTreeMap::from([("wire".into(), [1, 0])]),
                path_tolerance: 0.0,
            },
        )
        .unwrap();

        assert_eq!(layout.origin, [100.0, 198.0]);
        assert_eq!(layout.size, [20.0, 4.0]);
        assert_eq!(shape_bounds(&layout.layers["wire"]), [0.0, 0.0, 20.0, 4.0]);
    }

    #[test]
    fn offsets_each_side_of_a_bent_path() {
        let shapes = path_to_shapes(&path(1, 0, 4, &[(0, 0), (10, 0), (10, 10)]), 0.0).unwrap();

        assert_eq!(
            shapes,
            vec![vec![vec![
                [0.0, 2.0],
                [8.0, 2.0],
                [8.0, 10.0],
                [12.0, 10.0],
                [12.0, -2.0],
                [0.0, -2.0],
            ]]]
        );
    }

    #[test]
    fn keeps_one_offset_vertex_per_centreline_vertex() {
        let points = &[(0, 0), (10, 0), (17, 3), (20, 10), (20, 20)];
        let shapes = path_to_shapes(&path(1, 0, 4, points), 0.0).unwrap();

        assert_eq!(shapes[0][0].len(), points.len() * 2);
    }

    #[test]
    fn simplifies_the_centreline_with_a_bounded_tolerance() {
        let path = path(1, 0, 4, &[(0, 0), (5, 1), (10, 0), (15, -1), (20, 0)]);

        assert_eq!(path_to_shapes(&path, 0.0).unwrap()[0][0].len(), 10);
        assert_eq!(path_to_shapes(&path, 1.5).unwrap()[0][0].len(), 4);
    }

    #[test]
    fn honors_explicit_path_extensions() {
        let mut library = GdsLibrary::new("semi-example");
        let mut top = GdsStruct::new("TOP");
        top.elems.push(
            GdsPath {
                path_type: Some(4),
                begin_extn: Some(3),
                end_extn: Some(5),
                ..path(1, 0, 4, &[(100, 200), (120, 200)])
            }
            .into(),
        );
        library.structs.push(top);

        let mut bytes = Vec::new();
        library.write(&mut bytes).unwrap();
        let layout = extract(
            &bytes,
            GdsLayoutRequest {
                cell: "TOP".into(),
                layers: BTreeMap::from([("wire".into(), [1, 0])]),
                path_tolerance: 0.0,
            },
        )
        .unwrap();

        assert_eq!(layout.origin, [97.0, 198.0]);
        assert_eq!(layout.size, [28.0, 4.0]);
    }

    #[test]
    fn polygonizes_round_path_ends() {
        let shapes = path_to_shapes(
            &GdsPath {
                path_type: Some(1),
                ..path(1, 0, 4, &[(0, 0), (10, 0)])
            },
            0.0,
        )
        .unwrap();

        assert!(shapes.iter().flatten().map(Vec::len).sum::<usize>() > 4);
        assert_eq!(shape_bounds(&shapes), [-2.0, -2.0, 12.0, 2.0]);
    }

    #[test]
    fn rejects_unresolved_cell_references() {
        let mut library = GdsLibrary::new("semi-example");
        let child = GdsStruct::new("CHILD");
        let mut top = GdsStruct::new("TOP");
        top.elems.push(
            GdsStructRef {
                name: "CHILD".into(),
                xy: LibraryPoint::new(0, 0),
                ..Default::default()
            }
            .into(),
        );
        library.structs.extend([child, top]);

        let mut bytes = Vec::new();
        library.write(&mut bytes).unwrap();
        let error = extract(
            &bytes,
            GdsLayoutRequest {
                cell: "TOP".into(),
                layers: BTreeMap::from([("active".into(), [1, 0])]),
                path_tolerance: 0.0,
            },
        )
        .unwrap_err();

        assert!(error.contains("hierarchy is not supported yet"));
    }

    #[test]
    fn rejects_invalid_gds_bytes() {
        assert!(inspect(b"not a GDS file").is_err());
    }
}
