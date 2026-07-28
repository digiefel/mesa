use std::collections::{BTreeMap, BTreeSet};

use gds21::{GdsElement, GdsLibrary};
use serde::{Deserialize, Serialize};

type GdsPoint = [f64; 2];
type GdsContour = Vec<GdsPoint>;
type GdsShape = Vec<GdsContour>;
type GdsShapes = Vec<GdsShape>;

const ROUND_CAP_SEGMENTS: usize = 12;

fn default_scale() -> f64 {
    1.0
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub(crate) struct GdsPadding {
    pub(crate) left: f64,
    pub(crate) right: f64,
    pub(crate) front: f64,
    pub(crate) back: f64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GdsLayoutRequest {
    pub(crate) cell: String,
    pub(crate) layers: BTreeMap<String, [i16; 2]>,
    #[serde(default, rename = "path-tolerance")]
    pub(crate) path_tolerance: f64,
    #[serde(default, rename = "unit-meters")]
    pub(crate) unit_meters: Option<f64>,
    #[serde(default = "default_scale")]
    pub(crate) scale: f64,
    #[serde(default = "default_scale", rename = "scale-x")]
    pub(crate) scale_x: f64,
    #[serde(default = "default_scale", rename = "scale-y")]
    pub(crate) scale_y: f64,
    #[serde(default)]
    pub(crate) padding: GdsPadding,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct GdsLayout {
    pub(crate) origin: GdsPoint,
    pub(crate) size: GdsPoint,
    pub(crate) content_size: GdsPoint,
    pub(crate) offset: GdsPoint,
    pub(crate) padding: GdsPadding,
    pub(crate) unit_meters: f64,
    pub(crate) source_unit_meters: f64,
    pub(crate) scale: GdsPoint,
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

    let user_unit_meters = user_unit_meters(&library);
    Ok(GdsInfo {
        library: library.name,
        db_unit_meters: library.units.db_unit(),
        user_unit_meters,
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
    if !request.path_tolerance.is_finite() || !(0.0..=1.0).contains(&request.path_tolerance) {
        return Err(format!(
            "GDS path tolerance must be a fraction between 0 and 1; got {}",
            request.path_tolerance,
        ));
    }
    for (name, value) in [
        ("scale", request.scale),
        ("scale-x", request.scale_x),
        ("scale-y", request.scale_y),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(format!(
                "GDS {name} must be positive and finite; got {value}"
            ));
        }
    }
    for (name, value) in [
        ("left", request.padding.left),
        ("right", request.padding.right),
        ("front", request.padding.front),
        ("back", request.padding.back),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(format!(
                "GDS padding {name} must be non-negative and finite; got {value}"
            ));
        }
    }

    let mut layers = request
        .layers
        .keys()
        .map(|name| (name.clone(), GdsShapes::new()))
        .collect::<BTreeMap<_, _>>();
    let mut bounds: Option<[f64; 4]> = None;
    let source_unit_meters = user_unit_meters(&library);
    let unit_meters = request.unit_meters.unwrap_or(source_unit_meters);
    if !unit_meters.is_finite() || unit_meters <= 0.0 {
        return Err(format!(
            "GDS output unit must be positive and finite; got {unit_meters}"
        ));
    }
    let visual_scale = [
        request.scale * request.scale_x,
        request.scale * request.scale_y,
    ];
    let units_per_database_unit = [
        library.units.db_unit() / unit_meters * visual_scale[0],
        library.units.db_unit() / unit_meters * visual_scale[1],
    ];
    if units_per_database_unit
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err("GDS unit conversion and scaling overflowed".to_string());
    }

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
            GdsElement::GdsPath(path) => path_to_shapes(path, request.path_tolerance)?,
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

    let [min_x, min_y, max_x, max_y] = bounds
        .ok_or_else(|| "selected GDS layers contain no boundary or path geometry".to_string())?;
    let origin = [
        min_x * units_per_database_unit[0],
        min_y * units_per_database_unit[1],
    ];
    let content_size = [
        (max_x - min_x) * units_per_database_unit[0],
        (max_y - min_y) * units_per_database_unit[1],
    ];
    let offset = [request.padding.left, request.padding.front];
    let size = [
        content_size[0] + request.padding.left + request.padding.right,
        content_size[1] + request.padding.front + request.padding.back,
    ];
    if origin
        .iter()
        .chain(content_size.iter())
        .chain(size.iter())
        .any(|value| !value.is_finite())
    {
        return Err("GDS transformed bounds overflowed".to_string());
    }

    for shapes in layers.values_mut() {
        for shape in shapes {
            for contour in shape {
                for point in contour {
                    point[0] = point[0] * units_per_database_unit[0] - origin[0] + offset[0];
                    point[1] = point[1] * units_per_database_unit[1] - origin[1] + offset[1];
                }
            }
        }
    }

    Ok(GdsLayout {
        origin,
        size,
        content_size,
        offset,
        padding: request.padding,
        unit_meters,
        source_unit_meters,
        scale: visual_scale,
        layers,
    })
}

fn user_unit_meters(library: &GdsLibrary) -> f64 {
    // gds21 0.2.0 returns user-units per metre here, despite the method name
    // and documentation. Invert it to recover the user-unit size in metres.
    1.0 / library.units.user_unit()
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

fn path_to_shapes(path: &gds21::GdsPath, relative_tolerance: f64) -> Result<GdsShapes, String> {
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
    let mut points = simplify_polyline(points, width as f64 * relative_tolerance);

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

    fn assert_point_close(actual: GdsPoint, expected: GdsPoint) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 1e-12,
                "expected {expected}, got {actual}",
            );
        }
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
    fn extracts_named_boundary_layers_in_gds_user_units() {
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
                unit_meters: None,
                scale: 1.0,
                scale_x: 1.0,
                scale_y: 1.0,
                padding: GdsPadding::default(),
            },
        )
        .unwrap();

        assert_eq!(layout.unit_meters, 1e-6);
        assert_eq!(layout.source_unit_meters, 1e-6);
        assert_eq!(layout.scale, [1.0, 1.0]);
        assert_eq!(layout.origin, [0.1, 0.2]);
        assert_eq!(layout.size, [0.01, 0.006]);
        for (actual, expected) in layout.layers["gate"][0][0].iter().zip([
            [0.002, 0.001],
            [0.008, 0.001],
            [0.008, 0.005],
            [0.002, 0.005],
        ]) {
            assert_point_close(*actual, expected);
        }
    }

    #[test]
    fn converts_units_scales_and_pads_planar_geometry() {
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
        library.structs.push(top);

        let mut bytes = Vec::new();
        library.write(&mut bytes).unwrap();
        let layout = extract(
            &bytes,
            GdsLayoutRequest {
                cell: "TOP".into(),
                layers: BTreeMap::from([("active".into(), [1, 0])]),
                path_tolerance: 0.0,
                unit_meters: Some(1e-9),
                scale: 0.5,
                scale_x: 2.0,
                scale_y: 3.0,
                padding: GdsPadding {
                    left: 1.0,
                    right: 2.0,
                    front: 3.0,
                    back: 4.0,
                },
            },
        )
        .unwrap();

        assert_eq!(layout.source_unit_meters, 1e-6);
        assert_eq!(layout.unit_meters, 1e-9);
        assert_eq!(layout.scale, [1.0, 1.5]);
        assert_eq!(layout.origin, [100.0, 300.0]);
        assert_eq!(layout.content_size, [10.0, 9.0]);
        assert_eq!(layout.offset, [1.0, 3.0]);
        assert_eq!(
            layout.padding,
            GdsPadding {
                left: 1.0,
                right: 2.0,
                front: 3.0,
                back: 4.0,
            }
        );
        assert_eq!(layout.size, [13.0, 16.0]);
        assert_eq!(
            layout.layers["active"][0][0],
            [[1.0, 3.0], [11.0, 3.0], [11.0, 12.0], [1.0, 12.0]]
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
                unit_meters: None,
                scale: 1.0,
                scale_x: 1.0,
                scale_y: 1.0,
                padding: GdsPadding::default(),
            },
        )
        .unwrap();

        assert_eq!(layout.unit_meters, 1e-6);
        assert_eq!(layout.origin, [0.1, 0.198]);
        assert_eq!(layout.size, [0.02, 0.004]);
        let [min_x, min_y, max_x, max_y] = shape_bounds(&layout.layers["wire"]);
        assert_point_close([min_x, min_y], [0.0, 0.0]);
        assert_point_close([max_x, max_y], [0.02, 0.004]);
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
    fn simplifies_the_centreline_relative_to_path_width() {
        let path = path(1, 0, 4, &[(0, 0), (5, 1), (10, 0), (15, -1), (20, 0)]);

        assert_eq!(path_to_shapes(&path, 0.0).unwrap()[0][0].len(), 10);
        assert_eq!(path_to_shapes(&path, 0.375).unwrap()[0][0].len(), 4);
    }

    #[test]
    fn relative_tolerance_is_scale_independent() {
        let narrow = path(1, 0, 4, &[(0, 0), (5, 1), (10, 0), (15, -1), (20, 0)]);
        let wide = path(
            1,
            0,
            40,
            &[(0, 0), (50, 10), (100, 0), (150, -10), (200, 0)],
        );

        assert_eq!(
            path_to_shapes(&narrow, 0.2).unwrap()[0][0].len(),
            path_to_shapes(&wide, 0.2).unwrap()[0][0].len(),
        );
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
                unit_meters: None,
                scale: 1.0,
                scale_x: 1.0,
                scale_y: 1.0,
                padding: GdsPadding::default(),
            },
        )
        .unwrap();

        assert_eq!(layout.origin, [0.097, 0.198]);
        assert_eq!(layout.size, [0.028, 0.004]);
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
                unit_meters: None,
                scale: 1.0,
                scale_x: 1.0,
                scale_y: 1.0,
                padding: GdsPadding::default(),
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
