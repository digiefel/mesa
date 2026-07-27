use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::Overlay;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_float::int::point::IntPoint;
use i_overlay::i_shape::int::shape::IntShapes;
use serde::{Deserialize, Serialize};

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
