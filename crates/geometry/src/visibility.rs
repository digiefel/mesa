use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::Overlay;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_float::int::point::IntPoint;
use i_overlay::i_shape::int::shape::IntShapes;
use serde::Serialize;

use crate::topology::{
    AtomicEdge, EdgeKind, Face, Point3, WireVolume, scene_geometry_with_smooth_join_cosine,
};

const EPSILON: f64 = 1e-9;
const SURFACE_SCALE: f64 = 1000.0;

type Point2 = [f64; 2];
type Point3F = [f64; 3];
pub(crate) type ViewMatrix = [[f64; 3]; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum EdgeVisibility {
    Visible,
    Occluded,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct WireEdge {
    pub(crate) start: Point3F,
    pub(crate) end: Point3F,
    pub(crate) kind: EdgeKind,
    pub(crate) interior: bool,
    pub(crate) visibility: EdgeVisibility,
    pub(crate) faces: u32,
    pub(crate) materials: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct WireSurface {
    pub(crate) normal: Point3,
    pub(crate) material: u32,
    pub(crate) contours: Vec<Vec<Point3>>,
}

struct ProjectedFace {
    contours: Vec<Vec<Point2>>,
    normal: Point3F,
    constant: f64,
    low: Point2,
    high: Point2,
    material: u32,
}

pub(crate) fn validate_view(view: ViewMatrix) -> Result<(), String> {
    if view
        .iter()
        .flatten()
        .any(|component| !component.is_finite())
    {
        return Err("view matrix components must be finite".into());
    }
    for (index, row) in view.iter().enumerate() {
        let length = dot(*row, *row);
        if (length - 1.0).abs() > 1e-7 {
            return Err(format!(
                "view matrix row {index} must be unit length; squared length is {length}"
            ));
        }
        for other in view.iter().skip(index + 1) {
            if dot(*row, *other).abs() > 1e-7 {
                return Err("view matrix rows must be orthogonal".into());
            }
        }
    }
    Ok(())
}

pub(crate) fn scene_edges(
    volumes: &[WireVolume],
    view: ViewMatrix,
    smooth_join_cosine: f64,
) -> Vec<WireEdge> {
    let (faces, edges) =
        scene_geometry_with_smooth_join_cosine(volumes, smooth_join_cosine);
    let projected_faces: Vec<ProjectedFace> = faces
        .iter()
        .map(|face| ProjectedFace::new(face, view))
        .collect();
    edges
        .iter()
        .flat_map(|edge| split_by_visibility(edge, &projected_faces, view))
        .collect()
}

pub(crate) fn scene_surfaces(volumes: &[WireVolume], view: ViewMatrix) -> Vec<WireSurface> {
    let faces = crate::topology::scene_faces(volumes)
        .into_iter()
        .filter(|face| face.normal[2] >= 0)
        .collect::<Vec<_>>();
    let projected: Vec<ProjectedFace> = faces
        .iter()
        .map(|face| ProjectedFace::new(face, view))
        .collect();
    let projected_shapes: Vec<IntShapes<i64>> = projected
        .iter()
        .map(|face| projected_to_int_shapes(&face.contours))
        .collect();
    let mut result = Vec::new();

    for (index, face) in faces.iter().enumerate() {
        let mut occluders = Vec::new();
        for (other_index, other) in projected.iter().enumerate() {
            if index == other_index
                || !projected[index].bounds_overlap(other)
            {
                continue;
            }
            let occluded = nearer_projected_region(
                other,
                &projected[index],
                &projected_shapes[other_index],
            );
            if !occluded.is_empty() {
                occluders.push(occluded);
            }
        }
        let occluded = union_regions(occluders);
        let visible = if occluded.is_empty() {
            projected_shapes[index].clone()
        } else {
            let mut overlay = Overlay::with_shapes(&projected_shapes[index], &occluded);
            overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd)
        };

        for shape in visible {
            let depth = shape
                .iter()
                .flatten()
                .filter_map(|point| {
                    projected[index].depth_at([
                        point.x as f64 / SURFACE_SCALE,
                        point.y as f64 / SURFACE_SCALE,
                    ])
                })
                .sum::<f64>()
                / shape.iter().map(Vec::len).sum::<usize>() as f64;
            let contours = shape
                .into_iter()
                .map(|contour| {
                    contour
                        .into_iter()
                        .filter_map(|point| {
                            let screen = [
                                point.x as f64 / SURFACE_SCALE,
                                point.y as f64 / SURFACE_SCALE,
                            ];
                            let depth = projected[index].depth_at(screen)?;
                            Some(inverse_transform_point(
                                [screen[0], screen[1], depth],
                                view,
                            ))
                        })
                        .collect()
                })
                .collect();
            result.push((
                depth,
                WireSurface {
                    normal: face.normal,
                    material: face.material,
                    contours,
                },
            ));
        }
    }
    result.sort_by(|left, right| left.0.total_cmp(&right.0));
    result.into_iter().map(|(_, surface)| surface).collect()
}

fn nearer_projected_region(
    candidate: &ProjectedFace,
    surface: &ProjectedFace,
    candidate_shape: &IntShapes<i64>,
) -> IntShapes<i64> {
    let (Some(candidate_depth), Some(surface_depth)) =
        (candidate.depth_coefficients(), surface.depth_coefficients())
    else {
        return Vec::new();
    };
    let coefficients = [
        candidate_depth[0] - surface_depth[0],
        candidate_depth[1] - surface_depth[1],
        candidate_depth[2] - surface_depth[2],
    ];
    let low = [
        candidate.low[0].max(surface.low[0]),
        candidate.low[1].max(surface.low[1]),
    ];
    let high = [
        candidate.high[0].min(surface.high[0]),
        candidate.high[1].min(surface.high[1]),
    ];
    let depths = [
        coefficients[0] * low[0] + coefficients[1] * low[1] + coefficients[2],
        coefficients[0] * low[0] + coefficients[1] * high[1] + coefficients[2],
        coefficients[0] * high[0] + coefficients[1] * low[1] + coefficients[2],
        coefficients[0] * high[0] + coefficients[1] * high[1] + coefficients[2],
    ];
    if depths.iter().all(|depth| *depth <= EPSILON) {
        Vec::new()
    } else if depths.iter().all(|depth| *depth > EPSILON) {
        candidate_shape.clone()
    } else {
        nearer_overlap(candidate, surface, candidate_shape)
    }
}

fn union_regions(mut regions: Vec<IntShapes<i64>>) -> IntShapes<i64> {
    while regions.len() > 1 {
        let mut merged = Vec::with_capacity(regions.len().div_ceil(2));
        let mut iterator = regions.into_iter();
        while let Some(first) = iterator.next() {
            if let Some(second) = iterator.next() {
                let mut overlay = Overlay::with_shapes(&first, &second);
                merged.push(overlay.overlay(OverlayRule::Union, FillRule::EvenOdd));
            } else {
                merged.push(first);
            }
        }
        regions = merged;
    }
    regions.pop().unwrap_or_default()
}

fn projected_to_int_shapes(contours: &[Vec<Point2>]) -> IntShapes<i64> {
    vec![
        contours
            .iter()
            .map(|contour| {
                contour
                    .iter()
                    .map(|[x, y]| {
                        IntPoint::new(
                            (x * SURFACE_SCALE).round() as i64,
                            (y * SURFACE_SCALE).round() as i64,
                        )
                    })
                    .collect()
            })
            .collect(),
    ]
}

fn nearer_overlap(
    candidate: &ProjectedFace,
    surface: &ProjectedFace,
    overlap: &IntShapes<i64>,
) -> IntShapes<i64> {
    let (Some(candidate_depth), Some(surface_depth)) =
        (candidate.depth_coefficients(), surface.depth_coefficients())
    else {
        return Vec::new();
    };
    let coefficients = [
        candidate_depth[0] - surface_depth[0],
        candidate_depth[1] - surface_depth[1],
        candidate_depth[2] - surface_depth[2],
    ];
    let gradient_length = coefficients[0].hypot(coefficients[1]);
    if gradient_length <= EPSILON {
        return if coefficients[2] > EPSILON {
            overlap.clone()
        } else {
            Vec::new()
        };
    }

    let normal = [
        coefficients[0] / gradient_length,
        coefficients[1] / gradient_length,
    ];
    let tangent = [normal[1], -normal[0]];
    let line_origin = [
        -coefficients[2] * coefficients[0] / gradient_length.powi(2),
        -coefficients[2] * coefficients[1] / gradient_length.powi(2),
    ];
    let radius = overlap
        .iter()
        .flatten()
        .flatten()
        .map(|point| {
            let x = point.x as f64 / SURFACE_SCALE - line_origin[0];
            let y = point.y as f64 / SURFACE_SCALE - line_origin[1];
            x.hypot(y)
        })
        .fold(1.0_f64, f64::max)
        * 4.0;
    let point = |along: f64, across: f64| {
        IntPoint::new(
            ((line_origin[0] + tangent[0] * along + normal[0] * across) * SURFACE_SCALE)
                .round() as i64,
            ((line_origin[1] + tangent[1] * along + normal[1] * across) * SURFACE_SCALE)
                .round() as i64,
        )
    };
    let positive_half_plane = vec![vec![vec![
        point(-radius, 0.0),
        point(radius, 0.0),
        point(radius, radius * 2.0),
        point(-radius, radius * 2.0),
    ]]];
    let mut overlay = Overlay::with_shapes(overlap, &positive_half_plane);
    overlay.overlay(OverlayRule::Intersect, FillRule::EvenOdd)
}

fn inverse_transform_point(point: Point3F, view: ViewMatrix) -> Point3 {
    [
        (view[0][0] * point[0] + view[1][0] * point[1] + view[2][0] * point[2])
            .round() as i64,
        (view[0][1] * point[0] + view[1][1] * point[1] + view[2][1] * point[2])
            .round() as i64,
        (view[0][2] * point[0] + view[1][2] * point[1] + view[2][2] * point[2])
            .round() as i64,
    ]
}

impl ProjectedFace {
    fn new(face: &Face, view: ViewMatrix) -> Self {
        let transformed_contours: Vec<Vec<Point3F>> = face
            .contours
            .iter()
            .map(|contour| {
                contour
                    .iter()
                    .map(|point| transform_point(*point, view))
                    .collect()
            })
            .collect();
        let contours: Vec<Vec<Point2>> = transformed_contours
            .iter()
            .map(|contour| contour.iter().map(|[x, y, _]| [*x, *y]).collect())
            .collect();
        let first = transformed_contours[0][0];
        let normal = transform_vector(face.normal, view);
        let constant = dot(normal, first);
        let mut points = contours.iter().flatten();
        let first = *points.next().expect("validated faces have contours");
        let mut low = first;
        let mut high = first;
        for point in points {
            low[0] = low[0].min(point[0]);
            low[1] = low[1].min(point[1]);
            high[0] = high[0].max(point[0]);
            high[1] = high[1].max(point[1]);
        }
        Self {
            contours,
            normal,
            constant,
            low,
            high,
            material: face.material,
        }
    }

    fn depth_at(&self, point: Point2) -> Option<f64> {
        if self.normal[2].abs() <= EPSILON {
            return None;
        }
        Some(
            (self.constant - self.normal[0] * point[0] - self.normal[1] * point[1])
                / self.normal[2],
        )
    }

    fn depth_coefficients(&self) -> Option<[f64; 3]> {
        if self.normal[2].abs() <= EPSILON {
            return None;
        }
        Some([
            -self.normal[0] / self.normal[2],
            -self.normal[1] / self.normal[2],
            self.constant / self.normal[2],
        ])
    }

    fn bounds_overlap(&self, other: &Self) -> bool {
        self.low[0] <= other.high[0] + EPSILON
            && self.high[0] + EPSILON >= other.low[0]
            && self.low[1] <= other.high[1] + EPSILON
            && self.high[1] + EPSILON >= other.low[1]
    }

    fn contains(&self, point: Point2) -> bool {
        if point[0] < self.low[0] - EPSILON
            || point[0] > self.high[0] + EPSILON
            || point[1] < self.low[1] - EPSILON
            || point[1] > self.high[1] + EPSILON
        {
            return false;
        }
        point_in_compound(point, &self.contours)
    }
}

fn split_by_visibility(
    edge: &AtomicEdge,
    faces: &[ProjectedFace],
    view: ViewMatrix,
) -> Vec<WireEdge> {
    let start = transform_point(edge.start, view);
    let end = transform_point(edge.end, view);
    let start_2d = [start[0], start[1]];
    let end_2d = [end[0], end[1]];
    if squared_length(subtract_2d(end_2d, start_2d)) <= EPSILON * EPSILON {
        return Vec::new();
    }

    let mut parameters = vec![0.0, 1.0];
    for (face_index, face) in faces.iter().enumerate() {
        if edge.incident.contains(&face_index) {
            continue;
        }
        for contour in &face.contours {
            for index in 0..contour.len() {
                parameters.extend(segment_intersections(
                    start_2d,
                    end_2d,
                    contour[index],
                    contour[(index + 1) % contour.len()],
                ));
            }
        }
        let start_distance = plane_distance(face, start);
        let end_distance = plane_distance(face, end);
        let denominator = start_distance - end_distance;
        if denominator.abs() > EPSILON {
            let parameter = start_distance / denominator;
            if parameter > EPSILON && parameter < 1.0 - EPSILON {
                parameters.push(parameter);
            }
        }
    }
    parameters.sort_by(f64::total_cmp);
    parameters.dedup_by(|left, right| (*left - *right).abs() <= EPSILON);

    let mut result: Vec<WireEdge> = Vec::new();
    for interval in parameters.windows(2) {
        let start_parameter = interval[0];
        let end_parameter = interval[1];
        if end_parameter - start_parameter <= EPSILON {
            continue;
        }
        let midpoint = (start_parameter + end_parameter) / 2.0;
        let transformed_midpoint = lerp(start, end, midpoint);
        let screen_midpoint = [transformed_midpoint[0], transformed_midpoint[1]];
        let occluded = faces.iter().enumerate().any(|(face_index, face)| {
            if edge.incident.contains(&face_index) || !face.contains(screen_midpoint) {
                return false;
            }
            let Some(face_depth) = face.depth_at(screen_midpoint) else {
                return false;
            };
            let scale = face_depth.abs().max(transformed_midpoint[2].abs()).max(1.0);
            face_depth - transformed_midpoint[2] > EPSILON * scale
        });
        let visibility = if occluded {
            EdgeVisibility::Occluded
        } else {
            EdgeVisibility::Visible
        };
        let segment = WireEdge {
            start: lerp_i64(edge.start, edge.end, start_parameter),
            end: lerp_i64(edge.start, edge.end, end_parameter),
            kind: edge.kind,
            interior: edge.interior,
            visibility,
            faces: edge.incident.len() as u32,
            materials: edge
                .incident
                .iter()
                .map(|face| faces[*face].material)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
        };
        if let Some(previous) = result.last_mut()
            && previous.visibility == segment.visibility
            && previous.kind == segment.kind
            && same_point(previous.end, segment.start)
        {
            previous.end = segment.end;
        } else {
            result.push(segment);
        }
    }
    result
}

fn segment_intersections(start: Point2, end: Point2, left: Point2, right: Point2) -> Vec<f64> {
    let direction = subtract_2d(end, start);
    let other_direction = subtract_2d(right, left);
    let offset = subtract_2d(left, start);
    let denominator = cross_2d(direction, other_direction);
    let scale = squared_length(direction)
        .sqrt()
        .max(squared_length(other_direction).sqrt())
        .max(1.0);

    if denominator.abs() > EPSILON * scale * scale {
        let parameter = cross_2d(offset, other_direction) / denominator;
        let other_parameter = cross_2d(offset, direction) / denominator;
        if (-EPSILON..=1.0 + EPSILON).contains(&parameter)
            && (-EPSILON..=1.0 + EPSILON).contains(&other_parameter)
        {
            return vec![parameter.clamp(0.0, 1.0)];
        }
        return Vec::new();
    }

    if cross_2d(offset, direction).abs() > EPSILON * scale * scale {
        return Vec::new();
    }
    let length_squared = squared_length(direction);
    if length_squared <= EPSILON * EPSILON {
        return Vec::new();
    }
    let first = dot_2d(subtract_2d(left, start), direction) / length_squared;
    let second = dot_2d(subtract_2d(right, start), direction) / length_squared;
    let low = first.min(second).max(0.0);
    let high = first.max(second).min(1.0);
    if low <= high + EPSILON {
        vec![low, high]
    } else {
        Vec::new()
    }
}

fn point_in_compound(point: Point2, contours: &[Vec<Point2>]) -> bool {
    let mut inside = false;
    for contour in contours {
        for index in 0..contour.len() {
            let start = contour[index];
            let end = contour[(index + 1) % contour.len()];
            if point_on_segment(point, start, end) {
                return true;
            }
            if (start[1] > point[1]) != (end[1] > point[1]) {
                let crossing =
                    start[0] + (point[1] - start[1]) * (end[0] - start[0]) / (end[1] - start[1]);
                if point[0] < crossing {
                    inside = !inside;
                }
            }
        }
    }
    inside
}

fn point_on_segment(point: Point2, start: Point2, end: Point2) -> bool {
    let direction = subtract_2d(end, start);
    let offset = subtract_2d(point, start);
    let scale = squared_length(direction).sqrt().max(1.0);
    cross_2d(direction, offset).abs() <= EPSILON * scale * scale
        && dot_2d(offset, direction) >= -EPSILON
        && dot_2d(subtract_2d(point, end), direction) <= EPSILON
}

fn plane_distance(face: &ProjectedFace, point: Point3F) -> f64 {
    dot(face.normal, point) - face.constant
}

fn transform_point(point: Point3, view: ViewMatrix) -> Point3F {
    let point = [point[0] as f64, point[1] as f64, point[2] as f64];
    [
        dot(view[0], point),
        dot(view[1], point),
        dot(view[2], point),
    ]
}

fn transform_vector(vector: Point3, view: ViewMatrix) -> Point3F {
    transform_point(vector, view)
}

fn lerp(start: Point3F, end: Point3F, parameter: f64) -> Point3F {
    [
        start[0] + parameter * (end[0] - start[0]),
        start[1] + parameter * (end[1] - start[1]),
        start[2] + parameter * (end[2] - start[2]),
    ]
}

fn lerp_i64(start: Point3, end: Point3, parameter: f64) -> Point3F {
    [
        start[0] as f64 + parameter * (end[0] - start[0]) as f64,
        start[1] as f64 + parameter * (end[1] - start[1]) as f64,
        start[2] as f64 + parameter * (end[2] - start[2]) as f64,
    ]
}

fn same_point(left: Point3F, right: Point3F) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| (*left - right).abs() <= EPSILON)
}

fn subtract_2d(left: Point2, right: Point2) -> Point2 {
    [left[0] - right[0], left[1] - right[1]]
}

fn cross_2d(left: Point2, right: Point2) -> f64 {
    left[0] * right[1] - left[1] * right[0]
}

fn dot_2d(left: Point2, right: Point2) -> f64 {
    left[0] * right[0] + left[1] * right[1]
}

fn squared_length(vector: Point2) -> f64 {
    dot_2d(vector, vector)
}

fn dot(left: Point3F, right: Point3F) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{from_int_shapes, to_int_shapes};

    #[test]
    fn splits_an_edge_into_visible_and_occluded_intervals() {
        let volumes = vec![
            WireVolume {
                shapes: vec![vec![rectangle(0, 0, 10_000, 2_000)]],
                bottom: 0,
                top: 1_000,
                material: 0,
                top_bevel: 0,
            },
            WireVolume {
                shapes: vec![vec![rectangle(4_000, -1_000, 6_000, 3_000)]],
                bottom: 2_000,
                top: 3_000,
                material: 1,
                top_bevel: 0,
            },
        ];
        let edges = scene_edges(
            &volumes,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            1.0,
        );
        let mut edge = edges
            .iter()
            .filter(|edge| {
                edge.kind == EdgeKind::Crease
                    && edge.start[1].abs() <= EPSILON
                    && edge.end[1].abs() <= EPSILON
                    && (edge.start[2] - 1_000.0).abs() <= EPSILON
                    && (edge.end[2] - 1_000.0).abs() <= EPSILON
            })
            .collect::<Vec<_>>();
        edge.sort_by(|left, right| left.start[0].total_cmp(&right.start[0]));

        assert_eq!(edge.len(), 3);
        assert_eq!(edge[0].visibility, EdgeVisibility::Visible);
        assert_eq!(edge[1].visibility, EdgeVisibility::Occluded);
        assert_eq!(edge[2].visibility, EdgeVisibility::Visible);
        assert_eq!((edge[0].start[0], edge[0].end[0]), (0.0, 4_000.0));
        assert_eq!((edge[1].start[0], edge[1].end[0]), (4_000.0, 6_000.0));
        assert_eq!((edge[2].start[0], edge[2].end[0]), (6_000.0, 10_000.0));
    }

    #[test]
    fn keeps_visible_outer_and_concave_vertical_outlines() {
        let subject = vec![vec![rectangle(0, 0, 10_000, 6_000)]];
        let result = vec![
            vec![
                rectangle(0, 0, 6_000, 6_000),
                rectangle(2_000, 1_000, 4_000, 3_000)
                    .into_iter()
                    .rev()
                    .collect(),
            ],
            vec![rectangle(7_000, 0, 10_000, 6_000)],
        ];
        let volumes = vec![
            WireVolume {
                shapes: subject,
                bottom: -1_500,
                top: 0,
                material: 0,
                top_bevel: 0,
            },
            WireVolume {
                shapes: result,
                bottom: 0,
                top: 1_500,
                material: 1,
                top_bevel: 0,
            },
        ];
        let view = cetz_ortho_view(35.0_f64.to_radians(), 35.0_f64.to_radians());
        let edges = scene_edges(&volumes, view, 1.0);

        assert_eq!(
            visibility_of(&edges, [0.0, 0.0, -1_500.0], [0.0, 0.0, 0.0]),
            Some(EdgeVisibility::Visible)
        );
        assert_eq!(
            visibility_of(&edges, [4_000.0, 3_000.0, 0.0], [4_000.0, 3_000.0, 1_500.0],),
            Some(EdgeVisibility::Occluded)
        );
        assert_eq!(
            visibility_of(
                &edges,
                [10_000.0, 0.0, -1_500.0],
                [10_000.0, 0.0, 0.0],
            ),
            Some(EdgeVisibility::Visible)
        );
        assert_eq!(
            visibility_of(
                &edges,
                [10_000.0, 0.0, 0.0],
                [10_000.0, 0.0, 1_500.0],
            ),
            Some(EdgeVisibility::Visible)
        );
    }

    #[test]
    fn clips_farther_faces_out_of_projected_contact_surfaces() {
        let bounds = vec![vec![rectangle(0, 0, 120_000, 50_000)]];
        let contacts = vec![
            vec![rectangle(15_000, 10_000, 45_000, 40_000)],
            vec![rectangle(75_000, 10_000, 105_000, 40_000)],
        ];
        let mut overlay =
            Overlay::with_shapes(&to_int_shapes(bounds.clone()), &to_int_shapes(contacts.clone()));
        let oxide =
            from_int_shapes(overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd));
        let volumes = vec![
            WireVolume {
                shapes: bounds,
                bottom: 0,
                top: 40_000,
                material: 0,
                top_bevel: 0,
            },
            WireVolume {
                shapes: oxide,
                bottom: 40_000,
                top: 45_000,
                material: 1,
                top_bevel: 0,
            },
            WireVolume {
                shapes: contacts,
                bottom: 40_000,
                top: 50_000,
                material: 2,
                top_bevel: 0,
            },
        ];
        let view = cetz_ortho_view(35.0_f64.to_radians(), 35.0_f64.to_radians());
        let surfaces = scene_surfaces(&volumes, view);
        let oxide = projected_surface_shapes(&surfaces, view, |surface| {
            surface.material == 1 && surface.normal == [0, 0, 1]
        });
        let contacts = projected_surface_shapes(&surfaces, view, |surface| {
            surface.material == 2 && surface.normal == [0, 0, 1]
        });
        let mut overlay = Overlay::with_shapes(&oxide, &contacts);
        let overlap = overlay.overlay(OverlayRule::Intersect, FillRule::EvenOdd);

        assert!(shape_area2(&overlap) * 10_000 < shape_area2(&contacts));

        let contact_sides = projected_surface_shapes(&surfaces, view, |surface| {
            surface.material == 2 && surface.normal[2] == 0
        });
        let mut overlay = Overlay::with_shapes(&oxide, &contact_sides);
        let overlap = overlay.overlay(OverlayRule::Intersect, FillRule::EvenOdd);
        assert!(shape_area2(&overlap) * 10_000 < shape_area2(&contact_sides));
    }

    #[test]
    fn subtracting_a_union_matches_sequential_occluder_subtraction() {
        let subject = to_int_shapes(vec![vec![rectangle(0, 0, 10_000, 6_000)]]);
        let occluders = vec![
            to_int_shapes(vec![vec![rectangle(1_000, -1_000, 6_000, 4_000)]]),
            to_int_shapes(vec![vec![rectangle(4_000, 2_000, 9_000, 7_000)]]),
        ];
        let mut sequential = subject.clone();
        for occluder in &occluders {
            let mut overlay = Overlay::with_shapes(&sequential, occluder);
            sequential = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);
        }
        let union = union_regions(occluders);
        let mut overlay = Overlay::with_shapes(&subject, &union);
        let combined = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);

        let mut overlay = Overlay::with_shapes(&sequential, &combined);
        let only_sequential = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);
        let mut overlay = Overlay::with_shapes(&combined, &sequential);
        let only_combined = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);
        assert_eq!(shape_area2(&only_sequential), 0);
        assert_eq!(shape_area2(&only_combined), 0);
    }

    fn projected_surface_shapes(
        surfaces: &[WireSurface],
        view: ViewMatrix,
        include: impl Fn(&WireSurface) -> bool,
    ) -> IntShapes<i64> {
        let mut result = Vec::new();
        for surface in surfaces.iter().filter(|surface| include(surface)) {
            let contours: Vec<Vec<Point2>> = surface
                .contours
                .iter()
                .map(|contour| {
                    contour
                        .iter()
                        .map(|point| {
                            let [x, y, _] = transform_point(*point, view);
                            [x, y]
                        })
                        .collect()
                })
                .collect();
            let shapes = projected_to_int_shapes(&contours);
            if result.is_empty() {
                result = shapes;
            } else {
                let mut overlay = Overlay::with_shapes(&result, &shapes);
                result = overlay.overlay(OverlayRule::Union, FillRule::EvenOdd);
            }
        }
        result
    }

    fn shape_area2(shapes: &IntShapes<i64>) -> i128 {
        shapes
            .iter()
            .map(|shape| {
                shape
                    .iter()
                    .map(|contour| {
                        contour
                            .iter()
                            .zip(contour.iter().cycle().skip(1))
                            .map(|(start, end)| {
                                i128::from(start.x) * i128::from(end.y)
                                    - i128::from(start.y) * i128::from(end.x)
                            })
                            .sum::<i128>()
                    })
                    .sum::<i128>()
                    .abs()
            })
            .sum()
    }

    fn visibility_of(edges: &[WireEdge], start: Point3F, end: Point3F) -> Option<EdgeVisibility> {
        edges
            .iter()
            .find(|edge| {
                (same_point(edge.start, start) && same_point(edge.end, end))
                    || (same_point(edge.start, end) && same_point(edge.end, start))
            })
            .map(|edge| edge.visibility)
    }

    fn cetz_ortho_view(x: f64, y: f64) -> ViewMatrix {
        let (sine_x, cosine_x) = x.sin_cos();
        let (sine_y, cosine_y) = y.sin_cos();
        [
            [cosine_y, sine_y, 0.0],
            [-sine_x * sine_y, sine_x * cosine_y, cosine_x],
            [cosine_x * sine_y, -cosine_x * cosine_y, sine_x],
        ]
    }

    fn rectangle(left: i64, bottom: i64, right: i64, top: i64) -> Vec<[i64; 2]> {
        vec![[left, bottom], [right, bottom], [right, top], [left, top]]
    }
}
