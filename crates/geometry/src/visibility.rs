use serde::Serialize;

use crate::topology::{AtomicEdge, EdgeKind, Face, Point3, WireVolume, scene_geometry};

const EPSILON: f64 = 1e-9;

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
}

struct ProjectedFace {
    contours: Vec<Vec<Point2>>,
    normal: Point3F,
    constant: f64,
    low: Point2,
    high: Point2,
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

pub(crate) fn scene_edges(volumes: &[WireVolume], view: ViewMatrix) -> Vec<WireEdge> {
    let (faces, edges) = scene_geometry(volumes);
    let projected_faces: Vec<ProjectedFace> = faces
        .iter()
        .map(|face| ProjectedFace::new(face, view))
        .collect();
    edges
        .iter()
        .flat_map(|edge| split_by_visibility(edge, &projected_faces, view))
        .collect()
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

    #[test]
    fn splits_an_edge_into_visible_and_occluded_intervals() {
        let volumes = vec![
            WireVolume {
                shapes: vec![vec![rectangle(0, 0, 10_000, 2_000)]],
                bottom: 0,
                top: 1_000,
                material: 0,
            },
            WireVolume {
                shapes: vec![vec![rectangle(4_000, -1_000, 6_000, 3_000)]],
                bottom: 2_000,
                top: 3_000,
                material: 1,
            },
        ];
        let edges = scene_edges(
            &volumes,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
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
            },
            WireVolume {
                shapes: result,
                bottom: 0,
                top: 1_500,
                material: 1,
            },
        ];
        let view = cetz_ortho_view(35.0_f64.to_radians(), 35.0_f64.to_radians());
        let edges = scene_edges(&volumes, view);

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
