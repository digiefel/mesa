use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::Overlay;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_float::int::point::IntPoint;
use i_overlay::i_shape::int::shape::IntShapes;
use i_overlay::string::clip::{ClipRule, IntClip};
use serde::{Deserialize, Serialize};

mod gds;
mod topology;
mod visibility;

#[cfg(target_arch = "wasm32")]
use wasm_minimal_protocol::wasm_func;

#[cfg(target_arch = "wasm32")]
wasm_minimal_protocol::initiate_protocol!();

const PROTOCOL_VERSION: u8 = 1;

type WirePoint = [i64; 2];
type WireContour = Vec<WirePoint>;
type WireShape = Vec<WireContour>;
type WireShapes = Vec<WireShape>;

#[derive(Debug, Deserialize, Serialize)]
struct DifferenceRequest {
    version: u8,
    subject: WireShapes,
    mask: WireShapes,
}

#[derive(Debug, Deserialize, Serialize)]
struct MergeRequest {
    version: u8,
    shapes: WireShapes,
}

#[derive(Debug, Deserialize, Serialize)]
struct GeometryResponse {
    version: u8,
    shapes: WireShapes,
}

#[derive(Debug, Deserialize, Serialize)]
struct CrossSectionRequest {
    version: u8,
    shapes: WireShapes,
    y: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct CrossSectionResponse {
    version: u8,
    intervals: Vec<[i64; 2]>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ClipYRequest {
    version: u8,
    shapes: WireShapes,
    y: i64,
    positive: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct ClipLineRequest {
    version: u8,
    shapes: WireShapes,
    from: WirePoint,
    to: WirePoint,
    keep_left: bool,
}

#[derive(Debug, Deserialize)]
struct SceneTopologyRequest {
    version: u8,
    volumes: Vec<topology::WireVolume>,
    view: visibility::ViewMatrix,
    #[serde(rename = "smooth-join-cosine")]
    smooth_join_cosine: f64,
}

#[derive(Debug, Serialize)]
struct SceneTopologyResponse {
    version: u8,
    edges: Vec<visibility::WireEdge>,
}

#[derive(Debug, Deserialize)]
struct SceneSurfacesRequest {
    version: u8,
    volumes: Vec<topology::WireVolume>,
    view: visibility::ViewMatrix,
}

#[derive(Debug, Serialize)]
struct SceneSurfacesResponse {
    version: u8,
    faces: Vec<visibility::WireSurface>,
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn kernel_version() -> Vec<u8> {
    env!("CARGO_PKG_VERSION").as_bytes().to_vec()
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn gds_info(input: &[u8]) -> Result<Vec<u8>, String> {
    let info = gds::inspect(input)?;
    let mut output = Vec::new();
    ciborium::into_writer(&(PROTOCOL_VERSION, info), &mut output)
        .map_err(|error| format!("could not encode GDS information: {error}"))?;
    Ok(output)
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn gds_layout(data: &[u8], input: &[u8]) -> Result<Vec<u8>, String> {
    let request: gds::GdsLayoutRequest =
        ciborium::from_reader(input).map_err(|error| format!("invalid request: {error}"))?;
    let layout = gds::extract(data, request)?;
    let mut output = Vec::new();
    ciborium::into_writer(&(PROTOCOL_VERSION, layout), &mut output)
        .map_err(|error| format!("could not encode GDS layout: {error}"))?;
    Ok(output)
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn difference(input: &[u8]) -> Result<Vec<u8>, String> {
    let request: DifferenceRequest =
        ciborium::from_reader(input).map_err(|error| format!("invalid request: {error}"))?;

    if request.version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported geometry protocol version {}; expected {}",
            request.version, PROTOCOL_VERSION
        ));
    }

    validate_shapes("subject", &request.subject)?;
    validate_shapes("mask", &request.mask)?;

    let subject = to_int_shapes(request.subject);
    let mask = to_int_shapes(request.mask);
    let mut overlay = Overlay::with_shapes(&subject, &mask);
    let result = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);

    encode_response(GeometryResponse {
        version: PROTOCOL_VERSION,
        shapes: from_int_shapes(result),
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn intersection(input: &[u8]) -> Result<Vec<u8>, String> {
    let request: DifferenceRequest =
        ciborium::from_reader(input).map_err(|error| format!("invalid request: {error}"))?;

    if request.version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported geometry protocol version {}; expected {}",
            request.version, PROTOCOL_VERSION
        ));
    }

    validate_shapes("subject", &request.subject)?;
    validate_shapes("mask", &request.mask)?;

    let subject = to_int_shapes(request.subject);
    let mask = to_int_shapes(request.mask);
    let mut overlay = Overlay::with_shapes(&subject, &mask);
    let result = overlay.overlay(OverlayRule::Intersect, FillRule::EvenOdd);

    encode_response(GeometryResponse {
        version: PROTOCOL_VERSION,
        shapes: from_int_shapes(result),
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn merge(input: &[u8]) -> Result<Vec<u8>, String> {
    let request: MergeRequest =
        ciborium::from_reader(input).map_err(|error| format!("invalid request: {error}"))?;

    if request.version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported geometry protocol version {}; expected {}",
            request.version, PROTOCOL_VERSION
        ));
    }

    validate_shapes("merge", &request.shapes)?;
    let mut result = IntShapes::new();
    for shape in to_int_shapes(request.shapes) {
        if result.is_empty() {
            result.push(shape);
        } else {
            let addition = vec![shape];
            let mut overlay = Overlay::with_shapes(&result, &addition);
            result = overlay.overlay(OverlayRule::Union, FillRule::EvenOdd);
        }
    }

    encode_response(GeometryResponse {
        version: PROTOCOL_VERSION,
        shapes: from_int_shapes(result),
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn cross_section(input: &[u8]) -> Result<Vec<u8>, String> {
    let request: CrossSectionRequest =
        ciborium::from_reader(input).map_err(|error| format!("invalid request: {error}"))?;

    if request.version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported geometry protocol version {}; expected {}",
            request.version, PROTOCOL_VERSION
        ));
    }

    validate_shapes("cross-section", &request.shapes)?;
    let shapes = to_int_shapes(request.shapes);
    let intervals = horizontal_intervals(&shapes, request.y);

    encode_cross_section(CrossSectionResponse {
        version: PROTOCOL_VERSION,
        intervals,
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn clip_y(input: &[u8]) -> Result<Vec<u8>, String> {
    let request: ClipYRequest =
        ciborium::from_reader(input).map_err(|error| format!("invalid request: {error}"))?;

    if request.version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported geometry protocol version {}; expected {}",
            request.version, PROTOCOL_VERSION
        ));
    }

    validate_shapes("clip-y", &request.shapes)?;
    let shapes = to_int_shapes(request.shapes);
    let result = clip_shapes_at_y(shapes, request.y, request.positive);

    encode_response(GeometryResponse {
        version: PROTOCOL_VERSION,
        shapes: from_int_shapes(result),
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn clip_line(input: &[u8]) -> Result<Vec<u8>, String> {
    let request: ClipLineRequest =
        ciborium::from_reader(input).map_err(|error| format!("invalid request: {error}"))?;

    if request.version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported geometry protocol version {}; expected {}",
            request.version, PROTOCOL_VERSION
        ));
    }

    validate_shapes("clip-line", &request.shapes)?;
    if request.from == request.to {
        return Err("clip-line requires two distinct points".into());
    }
    let shapes = to_int_shapes(request.shapes);
    let result = clip_shapes_at_line(
        shapes,
        IntPoint::new(request.from[0], request.from[1]),
        IntPoint::new(request.to[0], request.to[1]),
        request.keep_left,
    );

    encode_response(GeometryResponse {
        version: PROTOCOL_VERSION,
        shapes: from_int_shapes(result),
    })
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn scene_topology(input: &[u8]) -> Result<Vec<u8>, String> {
    let request: SceneTopologyRequest =
        ciborium::from_reader(input).map_err(|error| format!("invalid request: {error}"))?;

    if request.version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported geometry protocol version {}; expected {}",
            request.version, PROTOCOL_VERSION
        ));
    }

    topology::validate_volumes(&request.volumes)?;
    visibility::validate_view(request.view)?;
    if !request.smooth_join_cosine.is_finite()
        || !(0.0..=1.0).contains(&request.smooth_join_cosine)
    {
        return Err(format!(
            "smooth join cosine must be between 0 and 1; got {}",
            request.smooth_join_cosine,
        ));
    }
    let edges = visibility::scene_edges(
        &request.volumes,
        request.view,
        request.smooth_join_cosine,
    );

    let mut output = Vec::new();
    ciborium::into_writer(
        &SceneTopologyResponse {
            version: PROTOCOL_VERSION,
            edges,
        },
        &mut output,
    )
    .map_err(|error| format!("could not encode response: {error}"))?;
    Ok(output)
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn scene_surfaces(input: &[u8]) -> Result<Vec<u8>, String> {
    let request: SceneSurfacesRequest =
        ciborium::from_reader(input).map_err(|error| format!("invalid request: {error}"))?;

    if request.version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported geometry protocol version {}; expected {}",
            request.version, PROTOCOL_VERSION
        ));
    }

    topology::validate_volumes(&request.volumes)?;
    visibility::validate_view(request.view)?;
    let faces = visibility::scene_surfaces(&request.volumes, request.view);
    let mut output = Vec::new();
    ciborium::into_writer(
        &SceneSurfacesResponse {
            version: PROTOCOL_VERSION,
            faces,
        },
        &mut output,
    )
    .map_err(|error| format!("could not encode response: {error}"))?;
    Ok(output)
}

fn validate_shapes(name: &str, shapes: &WireShapes) -> Result<(), String> {
    for (shape_index, shape) in shapes.iter().enumerate() {
        if shape.is_empty() {
            return Err(format!("{name} shape {shape_index} has no contours"));
        }

        for (contour_index, contour) in shape.iter().enumerate() {
            if contour.len() < 3 {
                return Err(format!(
                    "{name} shape {shape_index} contour {contour_index} has fewer than 3 points"
                ));
            }
        }
    }

    Ok(())
}

fn to_int_shapes(shapes: WireShapes) -> IntShapes<i64> {
    shapes
        .into_iter()
        .map(|shape| {
            shape
                .into_iter()
                .map(|contour| {
                    contour
                        .into_iter()
                        .map(|[x, y]| IntPoint::new(x, y))
                        .collect()
                })
                .collect()
        })
        .collect()
}

fn from_int_shapes(shapes: IntShapes<i64>) -> WireShapes {
    shapes
        .into_iter()
        .map(|shape| {
            shape
                .into_iter()
                .map(|contour| {
                    contour
                        .into_iter()
                        .map(|point| [point.x, point.y])
                        .collect()
                })
                .collect()
        })
        .collect()
}

fn encode_response(response: GeometryResponse) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    ciborium::into_writer(&response, &mut output)
        .map_err(|error| format!("could not encode response: {error}"))?;
    Ok(output)
}

fn encode_cross_section(response: CrossSectionResponse) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    ciborium::into_writer(&response, &mut output)
        .map_err(|error| format!("could not encode response: {error}"))?;
    Ok(output)
}

fn horizontal_intervals(shapes: &IntShapes<i64>, y: i64) -> Vec<[i64; 2]> {
    let Some((minimum_x, maximum_x)) = x_bounds(shapes) else {
        return Vec::new();
    };
    let line = [
        IntPoint::new(minimum_x.saturating_sub(1), y),
        IntPoint::new(maximum_x.saturating_add(1), y),
    ];
    let mut intervals: Vec<[i64; 2]> = shapes
        .clip_line(
            line,
            FillRule::EvenOdd,
            ClipRule {
                invert: false,
                boundary_included: true,
            },
        )
        .into_iter()
        .filter_map(|path| {
            let left = path.iter().map(|point| point.x).min()?;
            let right = path.iter().map(|point| point.x).max()?;
            (left < right).then_some([left, right])
        })
        .collect();

    intervals.sort_unstable_by_key(|interval| interval[0]);
    merge_intervals(intervals)
}

fn x_bounds(shapes: &IntShapes<i64>) -> Option<(i64, i64)> {
    let mut points = shapes.iter().flatten().flatten();
    let first = points.next()?;
    let mut minimum = first.x;
    let mut maximum = first.x;
    for point in points {
        minimum = minimum.min(point.x);
        maximum = maximum.max(point.x);
    }
    Some((minimum, maximum))
}

fn bounds(shapes: &IntShapes<i64>) -> Option<(i64, i64, i64, i64)> {
    let mut points = shapes.iter().flatten().flatten();
    let first = points.next()?;
    let mut minimum_x = first.x;
    let mut minimum_y = first.y;
    let mut maximum_x = first.x;
    let mut maximum_y = first.y;
    for point in points {
        minimum_x = minimum_x.min(point.x);
        minimum_y = minimum_y.min(point.y);
        maximum_x = maximum_x.max(point.x);
        maximum_y = maximum_y.max(point.y);
    }
    Some((minimum_x, minimum_y, maximum_x, maximum_y))
}

fn clip_shapes_at_y(shapes: IntShapes<i64>, y: i64, positive: bool) -> IntShapes<i64> {
    clip_shapes_at_line(
        shapes,
        IntPoint::new(0, y),
        IntPoint::new(1, y),
        positive,
    )
}

fn clip_shapes_at_line(
    shapes: IntShapes<i64>,
    from: IntPoint<i64>,
    to: IntPoint<i64>,
    keep_left: bool,
) -> IntShapes<i64> {
    let Some((minimum_x, minimum_y, maximum_x, maximum_y)) = bounds(&shapes) else {
        return Vec::new();
    };

    let (from, to) = if keep_left { (from, to) } else { (to, from) };
    let rectangle = vec![
        IntPoint::new(minimum_x, minimum_y),
        IntPoint::new(maximum_x, minimum_y),
        IntPoint::new(maximum_x, maximum_y),
        IntPoint::new(minimum_x, maximum_y),
    ];
    let clipping_contour = clip_contour_to_left_half_plane(&rectangle, from, to);
    if clipping_contour.len() < 3 {
        return Vec::new();
    }
    if clipping_contour == rectangle {
        return shapes;
    }
    let clipping_shape = vec![vec![clipping_contour]];
    let mut overlay = Overlay::with_shapes(&shapes, &clipping_shape);
    overlay.overlay(OverlayRule::Intersect, FillRule::EvenOdd)
}

fn clip_contour_to_left_half_plane(
    contour: &[IntPoint<i64>],
    from: IntPoint<i64>,
    to: IntPoint<i64>,
) -> Vec<IntPoint<i64>> {
    let mut result = Vec::new();
    let mut start = *contour.last().expect("bounding rectangle is not empty");
    let mut start_side = line_side(from, to, start);
    for &end in contour {
        let end_side = line_side(from, to, end);
        let start_inside = start_side >= 0;
        let end_inside = end_side >= 0;
        if start_inside != end_inside {
            result.push(line_intersection(start, end, start_side, end_side));
        }
        if end_inside {
            result.push(end);
        }
        start = end;
        start_side = end_side;
    }
    result.dedup();
    if result.len() > 1 && result.first() == result.last() {
        result.pop();
    }
    result
}

fn line_side(from: IntPoint<i64>, to: IntPoint<i64>, point: IntPoint<i64>) -> i128 {
    (i128::from(to.x) - i128::from(from.x)) * (i128::from(point.y) - i128::from(from.y))
        - (i128::from(to.y) - i128::from(from.y))
            * (i128::from(point.x) - i128::from(from.x))
}

fn line_intersection(
    start: IntPoint<i64>,
    end: IntPoint<i64>,
    start_side: i128,
    end_side: i128,
) -> IntPoint<i64> {
    let denominator = start_side - end_side;
    let x = i128::from(start.x) * denominator
        + (i128::from(end.x) - i128::from(start.x)) * start_side;
    let y = i128::from(start.y) * denominator
        + (i128::from(end.y) - i128::from(start.y)) * start_side;
    IntPoint::new(
        rounded_division(x, denominator) as i64,
        rounded_division(y, denominator) as i64,
    )
}

fn rounded_division(numerator: i128, denominator: i128) -> i128 {
    let sign = if numerator.signum() == denominator.signum() {
        1
    } else {
        -1
    };
    sign * (numerator.abs() + denominator.abs() / 2) / denominator.abs()
}

fn merge_intervals(intervals: Vec<[i64; 2]>) -> Vec<[i64; 2]> {
    let mut merged: Vec<[i64; 2]> = Vec::new();
    for [left, right] in intervals {
        if let Some(last) = merged.last_mut()
            && left <= last[1]
        {
            last[1] = last[1].max(right);
        } else {
            merged.push([left, right]);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_package_version() {
        assert_eq!(kernel_version(), b"0.1.0");
    }

    #[test]
    fn subtracts_disconnected_and_enclosed_mask_regions() {
        let request = DifferenceRequest {
            version: PROTOCOL_VERSION,
            subject: vec![vec![rectangle(0, 0, 10, 6)]],
            mask: vec![vec![rectangle(2, 1, 4, 3)], vec![rectangle(6, -1, 7, 7)]],
        };
        let mut input = Vec::new();
        ciborium::into_writer(&request, &mut input).unwrap();

        let encoded = difference(&input).unwrap();
        let response: GeometryResponse = ciborium::from_reader(encoded.as_slice()).unwrap();

        assert_eq!(response.version, PROTOCOL_VERSION);
        assert_eq!(response.shapes.len(), 2);
        assert_eq!(
            response
                .shapes
                .iter()
                .map(|shape| shape.len())
                .sum::<usize>(),
            3
        );
        assert_eq!(twice_area(&response.shapes), 100);
    }

    #[test]
    fn intersects_polygon_regions() {
        let request = DifferenceRequest {
            version: PROTOCOL_VERSION,
            subject: vec![vec![rectangle(0, 0, 10, 6)]],
            mask: vec![vec![rectangle(4, -1, 12, 3)]],
        };
        let mut input = Vec::new();
        ciborium::into_writer(&request, &mut input).unwrap();

        let encoded = intersection(&input).unwrap();
        let response: GeometryResponse = ciborium::from_reader(encoded.as_slice()).unwrap();

        assert_eq!(response.version, PROTOCOL_VERSION);
        assert_eq!(response.shapes.len(), 1);
        assert_eq!(twice_area(&response.shapes).abs(), 36);
    }

    #[test]
    fn merges_overlapping_shapes_into_one_region() {
        let request = MergeRequest {
            version: PROTOCOL_VERSION,
            shapes: vec![vec![rectangle(0, 0, 10, 10)], vec![rectangle(5, 0, 15, 10)]],
        };
        let mut input = Vec::new();
        ciborium::into_writer(&request, &mut input).unwrap();

        let encoded = merge(&input).unwrap();
        let response: GeometryResponse = ciborium::from_reader(encoded.as_slice()).unwrap();

        assert_eq!(response.version, PROTOCOL_VERSION);
        assert_eq!(response.shapes.len(), 1);
        assert_eq!(twice_area(&response.shapes).abs(), 300);
    }

    #[test]
    fn slices_a_masked_polygon_into_intervals() {
        let subject = vec![vec![rectangle(0, 0, 10, 6)]];
        let mask = vec![vec![rectangle(2, 1, 4, 3)], vec![rectangle(6, -1, 7, 7)]];
        let mut overlay = Overlay::with_shapes(&to_int_shapes(subject), &to_int_shapes(mask));
        let shapes = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);

        assert_eq!(
            horizontal_intervals(&shapes, 2),
            vec![[0, 2], [4, 6], [7, 10]]
        );
        assert_eq!(horizontal_intervals(&shapes, 4), vec![[0, 6], [7, 10]]);
    }

    #[test]
    fn clips_a_masked_polygon_to_either_side_of_a_cut() {
        let subject = vec![vec![rectangle(0, 0, 10, 6)]];
        let mask = vec![vec![rectangle(2, 1, 4, 3)], vec![rectangle(6, -1, 7, 7)]];
        let mut overlay = Overlay::with_shapes(&to_int_shapes(subject), &to_int_shapes(mask));
        let shapes = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);

        let positive = from_int_shapes(clip_shapes_at_y(shapes.clone(), 2, true));
        assert_eq!(positive.len(), 2);
        assert_eq!(positive.iter().map(Vec::len).sum::<usize>(), 2);
        assert_eq!(twice_area(&positive), 68);

        let negative = from_int_shapes(clip_shapes_at_y(shapes, 2, false));
        assert_eq!(twice_area(&negative), 32);
    }

    #[test]
    fn clips_polygon_geometry_to_either_side_of_an_arbitrary_line() {
        let subject = vec![vec![rectangle(0, 0, 10, 6)]];
        let mask = vec![vec![rectangle(2, 1, 4, 3)], vec![rectangle(6, -1, 7, 7)]];
        let mut overlay = Overlay::with_shapes(&to_int_shapes(subject), &to_int_shapes(mask));
        let shapes = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);
        let from = IntPoint::new(0, 2);
        let to = IntPoint::new(10, 4);

        let left = from_int_shapes(clip_shapes_at_line(shapes.clone(), from, to, true));
        let right = from_int_shapes(clip_shapes_at_line(shapes, from, to, false));

        assert_eq!(twice_area(&left) + twice_area(&right), 100);
        assert_eq!(twice_area(&left), 53);
        assert_eq!(twice_area(&right), 47);
    }

    #[test]
    fn rejects_a_cut_line_without_direction() {
        let request = ClipLineRequest {
            version: PROTOCOL_VERSION,
            shapes: vec![vec![rectangle(0, 0, 10, 6)]],
            from: [3, 2],
            to: [3, 2],
            keep_left: true,
        };
        let mut input = Vec::new();
        ciborium::into_writer(&request, &mut input).unwrap();

        assert_eq!(
            clip_line(&input).unwrap_err(),
            "clip-line requires two distinct points"
        );
    }

    fn rectangle(left: i64, bottom: i64, right: i64, top: i64) -> WireContour {
        vec![[left, bottom], [right, bottom], [right, top], [left, top]]
    }

    fn twice_area(shapes: &WireShapes) -> i128 {
        shapes
            .iter()
            .flatten()
            .map(|contour| {
                contour
                    .iter()
                    .zip(contour.iter().cycle().skip(1))
                    .map(|([x0, y0], [x1, y1])| {
                        i128::from(*x0) * i128::from(*y1) - i128::from(*y0) * i128::from(*x1)
                    })
                    .sum::<i128>()
            })
            .sum()
    }
}
