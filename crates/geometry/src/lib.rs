use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::Overlay;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_float::int::point::IntPoint;
use i_overlay::i_shape::int::shape::IntShapes;
use i_overlay::string::clip::{ClipRule, IntClip};
use serde::{Deserialize, Serialize};

mod topology;

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

#[derive(Debug, Deserialize)]
struct SceneTopologyRequest {
    version: u8,
    volumes: Vec<topology::WireVolume>,
}

#[derive(Debug, Serialize)]
struct SceneTopologyResponse {
    version: u8,
    edges: Vec<topology::WireEdge>,
}

#[cfg_attr(target_arch = "wasm32", wasm_func)]
pub fn kernel_version() -> Vec<u8> {
    env!("CARGO_PKG_VERSION").as_bytes().to_vec()
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
    let edges = topology::scene_edges(&request.volumes);

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
    let Some((minimum_x, minimum_y, maximum_x, maximum_y)) = bounds(&shapes) else {
        return Vec::new();
    };

    if positive && y <= minimum_y || !positive && y >= maximum_y {
        return shapes;
    }
    if positive && y >= maximum_y || !positive && y <= minimum_y {
        return Vec::new();
    }

    let (bottom, top) = if positive {
        (y, maximum_y)
    } else {
        (minimum_y, y)
    };
    let clipping_shape = vec![vec![vec![
        IntPoint::new(minimum_x, bottom),
        IntPoint::new(maximum_x, bottom),
        IntPoint::new(maximum_x, top),
        IntPoint::new(minimum_x, top),
    ]]];
    let mut overlay = Overlay::with_shapes(&shapes, &clipping_shape);
    overlay.overlay(OverlayRule::Intersect, FillRule::EvenOdd)
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
