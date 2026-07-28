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
// Leave room for the half-plane polygons used while splitting crossing faces.
// Keeping coordinates below 2^31 also keeps i_overlay's grid shifts valid on wasm32.
const SURFACE_MAX_COORDINATE: f64 = i32::MAX as f64 / 16.0;

type Point2 = [f64; 2];
type Point3F = [f64; 3];
type SurfaceShapes = IntShapes<i32>;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<u32>,
    pub(crate) normal: Point3,
    pub(crate) material: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) center: Option<Point3F>,
    #[serde(rename = "light-visibility")]
    pub(crate) light_visibility: f64,
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

#[derive(Clone, Copy)]
struct SurfaceGrid {
    origin: Point2,
    scale: f64,
}

impl SurfaceGrid {
    fn new(faces: &[ProjectedFace]) -> Self {
        let Some(first) = faces.first() else {
            return Self {
                origin: [0.0, 0.0],
                scale: SURFACE_SCALE,
            };
        };
        let mut low = first.low;
        let mut high = first.high;
        for face in faces.iter().skip(1) {
            low[0] = low[0].min(face.low[0]);
            low[1] = low[1].min(face.low[1]);
            high[0] = high[0].max(face.high[0]);
            high[1] = high[1].max(face.high[1]);
        }
        let span = (high[0] - low[0]).max(high[1] - low[1]);
        let scale = if span > EPSILON {
            SURFACE_SCALE.min(SURFACE_MAX_COORDINATE / span)
        } else {
            SURFACE_SCALE
        };
        Self { origin: low, scale }
    }

    fn encode(self, point: Point2) -> IntPoint<i32> {
        let encode = |value: f64, origin: f64| {
            ((value - origin) * self.scale)
                .round()
                .clamp(i32::MIN as f64 + 1.0, i32::MAX as f64 - 1.0) as i32
        };
        IntPoint::new(
            encode(point[0], self.origin[0]),
            encode(point[1], self.origin[1]),
        )
    }

    fn decode(self, point: IntPoint<i32>) -> Point2 {
        [
            point.x as f64 / self.scale + self.origin[0],
            point.y as f64 / self.scale + self.origin[1],
        ]
    }
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
    let (faces, edges) = scene_geometry_with_smooth_join_cosine(volumes, smooth_join_cosine);
    let projected_faces: Vec<ProjectedFace> = faces
        .iter()
        .map(|face| ProjectedFace::new(face, view))
        .collect();
    edges
        .iter()
        .flat_map(|edge| split_by_visibility(edge, &projected_faces, view))
        .collect()
}

pub(crate) fn scene_surfaces(
    volumes: &[WireVolume],
    view: ViewMatrix,
    toward_light: Point3F,
    shadows: bool,
    diagnostics: bool,
) -> Vec<WireSurface> {
    let faces = crate::topology::scene_faces(volumes)
        .into_iter()
        .filter(|face| face.normal[0] != 0 || face.normal[1] != 0 || face.normal[2] >= 0)
        .collect::<Vec<_>>();
    let centers: Vec<Point3F> = faces.iter().map(face_center).collect();
    let projected: Vec<ProjectedFace> = faces
        .iter()
        .map(|face| ProjectedFace::new(face, view))
        .collect();
    let grid = SurfaceGrid::new(&projected);
    let projected_shapes: Vec<SurfaceShapes> = projected
        .iter()
        .map(|face| projected_to_int_shapes(&face.contours, grid))
        .collect();
    let mut result = Vec::new();

    for (index, face) in faces.iter().enumerate() {
        if projected_shapes[index].is_empty() {
            continue;
        }
        let mut occluders = Vec::new();
        for (other_index, other) in projected.iter().enumerate() {
            if index == other_index
                || projected_shapes[other_index].is_empty()
                || !projected[index].bounds_overlap(other)
            {
                continue;
            }
            let occluded = nearer_projected_region(
                other,
                &projected[index],
                &projected_shapes[other_index],
                grid,
            );
            if !occluded.is_empty() {
                occluders.push(occluded);
            }
        }
        let mut visible = projected_shapes[index].clone();
        for occluder in occluders {
            let mut overlay = Overlay::with_shapes(&visible, &occluder);
            visible = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);
            if visible.is_empty() {
                break;
            }
        }

        let light_regions = if shadows {
            split_by_light_visibility(index, &faces, &visible, toward_light, view, grid)
        } else {
            vec![(1.0, visible)]
        };
        for (light_visibility, shapes) in light_regions {
            for shape in shapes {
                let depth = shape
                    .iter()
                    .flatten()
                    .filter_map(|point| projected[index].depth_at(grid.decode(*point)))
                    .sum::<f64>()
                    / shape.iter().map(Vec::len).sum::<usize>() as f64;
                let contours = shape
                    .into_iter()
                    .map(|contour| {
                        contour
                            .into_iter()
                            .filter_map(|point| {
                                let screen = grid.decode(point);
                                let depth = projected[index].depth_at(screen)?;
                                Some(inverse_transform_point([screen[0], screen[1], depth], view))
                            })
                            .collect()
                    })
                    .collect();
                result.push((
                    depth,
                    WireSurface {
                        source: diagnostics.then_some(index as u32),
                        normal: face.normal,
                        material: face.material,
                        center: diagnostics.then_some(centers[index]),
                        light_visibility,
                        contours,
                    },
                ));
            }
        }
    }
    result.sort_by(|left, right| left.0.total_cmp(&right.0));
    result.into_iter().map(|(_, surface)| surface).collect()
}

fn face_center(face: &Face) -> Point3F {
    let Some(contour) = face.contours.first() else {
        return [0.0; 3];
    };
    if contour.is_empty() {
        return [0.0; 3];
    }
    let mut center = [0.0; 3];
    for point in contour {
        for axis in 0..3 {
            center[axis] += point[axis] as f64;
        }
    }
    for component in &mut center {
        *component /= contour.len() as f64;
    }
    center
}

fn split_by_light_visibility(
    receiver_index: usize,
    faces: &[Face],
    visible: &SurfaceShapes,
    toward_light: Point3F,
    view: ViewMatrix,
    grid: SurfaceGrid,
) -> Vec<(f64, SurfaceShapes)> {
    let receiver = &faces[receiver_index];
    let normal = unit_3d([
        receiver.normal[0] as f64,
        receiver.normal[1] as f64,
        receiver.normal[2] as f64,
    ]);
    let denominator = dot(normal, toward_light);
    if denominator <= EPSILON {
        return vec![(0.0, visible.clone())];
    }
    let Some(receiver_point) = receiver
        .contours
        .first()
        .and_then(|contour| contour.first())
    else {
        return Vec::new();
    };
    let receiver_point = [
        receiver_point[0] as f64,
        receiver_point[1] as f64,
        receiver_point[2] as f64,
    ];
    let visible_bounds = int_shapes_bounds(visible);
    let mut lit = visible.clone();
    let mut has_shadow = false;

    for (occluder_index, occluder) in faces.iter().enumerate() {
        if receiver_index == occluder_index {
            continue;
        }
        let shadow = projected_shadow(
            occluder,
            receiver_point,
            normal,
            denominator,
            toward_light,
            view,
            grid,
        );
        if shadow.is_empty() || !int_bounds_overlap(visible_bounds, int_shapes_bounds(&shadow)) {
            continue;
        }
        has_shadow = true;
        let mut overlay = Overlay::with_shapes(&lit, &shadow);
        lit = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);
        if lit.is_empty() {
            return vec![(0.0, visible.clone())];
        }
    }

    if !has_shadow {
        return vec![(1.0, visible.clone())];
    }
    let mut overlay = Overlay::with_shapes(visible, &lit);
    let shadowed = overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd);
    let mut result = Vec::new();
    if !lit.is_empty() {
        result.push((1.0, lit));
    }
    if !shadowed.is_empty() {
        result.push((0.0, shadowed));
    }
    result
}

fn projected_shadow(
    occluder: &Face,
    receiver_point: Point3F,
    receiver_normal: Point3F,
    denominator: f64,
    toward_light: Point3F,
    view: ViewMatrix,
    grid: SurfaceGrid,
) -> SurfaceShapes {
    let contours = occluder
        .contours
        .iter()
        .map(|contour| {
            clip_contour_above_plane(contour, receiver_point, receiver_normal)
                .into_iter()
                .map(|point| {
                    let distance = dot(receiver_normal, subtract_3d(point, receiver_point));
                    let projected =
                        subtract_3d(point, scale_3d(toward_light, distance / denominator));
                    let [x, y, _] = transform_point_f64(projected, view);
                    [x, y]
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    projected_to_int_shapes(&contours, grid)
}

fn clip_contour_above_plane(
    contour: &[Point3],
    plane_point: Point3F,
    plane_normal: Point3F,
) -> Vec<Point3F> {
    if contour.is_empty() {
        return Vec::new();
    }
    let point = |point: Point3| [point[0] as f64, point[1] as f64, point[2] as f64];
    let distance = |point: Point3F| dot(plane_normal, subtract_3d(point, plane_point));
    let mut result = Vec::new();
    let mut previous = point(*contour.last().unwrap());
    let mut previous_distance = distance(previous);
    let mut previous_inside = previous_distance > EPSILON;
    for &current in contour {
        let current = point(current);
        let current_distance = distance(current);
        let current_inside = current_distance > EPSILON;
        if current_inside != previous_inside {
            let parameter = previous_distance / (previous_distance - current_distance);
            result.push(lerp(previous, current, parameter));
        }
        if current_inside {
            result.push(current);
        }
        previous = current;
        previous_distance = current_distance;
        previous_inside = current_inside;
    }
    result
}

fn int_shapes_bounds(shapes: &SurfaceShapes) -> Option<[i32; 4]> {
    let mut points = shapes.iter().flatten().flatten();
    let first = points.next()?;
    let mut bounds = [first.x, first.y, first.x, first.y];
    for point in points {
        bounds[0] = bounds[0].min(point.x);
        bounds[1] = bounds[1].min(point.y);
        bounds[2] = bounds[2].max(point.x);
        bounds[3] = bounds[3].max(point.y);
    }
    Some(bounds)
}

fn int_bounds_overlap(left: Option<[i32; 4]>, right: Option<[i32; 4]>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left[0] <= right[2] && left[2] >= right[0] && left[1] <= right[3] && left[3] >= right[1]
        }
        _ => false,
    }
}

fn nearer_projected_region(
    candidate: &ProjectedFace,
    surface: &ProjectedFace,
    candidate_shape: &SurfaceShapes,
    grid: SurfaceGrid,
) -> SurfaceShapes {
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
        nearer_overlap(candidate, surface, candidate_shape, grid)
    }
}

fn projected_to_int_shapes(contours: &[Vec<Point2>], grid: SurfaceGrid) -> SurfaceShapes {
    let Some(outer) = contours
        .first()
        .and_then(|contour| projected_to_int_path(contour, grid))
    else {
        return Vec::new();
    };
    let mut shape = vec![outer];
    shape.extend(
        contours
            .iter()
            .skip(1)
            .filter_map(|contour| projected_to_int_path(contour, grid)),
    );
    vec![shape]
}

fn projected_to_int_path(contour: &[Point2], grid: SurfaceGrid) -> Option<Vec<IntPoint<i32>>> {
    let mut path = Vec::with_capacity(contour.len());
    for point in contour {
        let point = grid.encode(*point);
        if path.last() != Some(&point) {
            path.push(point);
        }
    }
    if path.len() > 1 && path.first() == path.last() {
        path.pop();
    }
    if path.len() < 3 || int_path_area2(&path) == 0 {
        None
    } else {
        Some(path)
    }
}

fn int_path_area2(path: &[IntPoint<i32>]) -> i128 {
    path.iter()
        .zip(path.iter().cycle().skip(1))
        .take(path.len())
        .map(|(left, right)| left.x as i128 * right.y as i128 - right.x as i128 * left.y as i128)
        .sum::<i128>()
}

fn nearer_overlap(
    candidate: &ProjectedFace,
    surface: &ProjectedFace,
    overlap: &SurfaceShapes,
    grid: SurfaceGrid,
) -> SurfaceShapes {
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
            let [x, y] = grid.decode(*point);
            let x = x - line_origin[0];
            let y = y - line_origin[1];
            x.hypot(y)
        })
        .fold(1.0_f64, f64::max)
        * 4.0;
    let point = |along: f64, across: f64| {
        grid.encode([
            line_origin[0] + tangent[0] * along + normal[0] * across,
            line_origin[1] + tangent[1] * along + normal[1] * across,
        ])
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
        (view[0][0] * point[0] + view[1][0] * point[1] + view[2][0] * point[2]).round() as i64,
        (view[0][1] * point[0] + view[1][1] * point[1] + view[2][1] * point[2]).round() as i64,
        (view[0][2] * point[0] + view[1][2] * point[1] + view[2][2] * point[2]).round() as i64,
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
    transform_point_f64([point[0] as f64, point[1] as f64, point[2] as f64], view)
}

fn transform_point_f64(point: Point3F, view: ViewMatrix) -> Point3F {
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

fn subtract_3d(left: Point3F, right: Point3F) -> Point3F {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale_3d(vector: Point3F, scale: f64) -> Point3F {
    [vector[0] * scale, vector[1] * scale, vector[2] * scale]
}

fn unit_3d(vector: Point3F) -> Point3F {
    let length = dot(vector, vector).sqrt();
    if length <= EPSILON {
        [0.0; 3]
    } else {
        scale_3d(vector, 1.0 / length)
    }
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
                bottom_bevel: 0,
            },
            WireVolume {
                shapes: vec![vec![rectangle(4_000, -1_000, 6_000, 3_000)]],
                bottom: 2_000,
                top: 3_000,
                material: 1,
                top_bevel: 0,
                bottom_bevel: 0,
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
                bottom_bevel: 0,
            },
            WireVolume {
                shapes: result,
                bottom: 0,
                top: 1_500,
                material: 1,
                top_bevel: 0,
                bottom_bevel: 0,
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
            visibility_of(&edges, [10_000.0, 0.0, -1_500.0], [10_000.0, 0.0, 0.0],),
            Some(EdgeVisibility::Visible)
        );
        assert_eq!(
            visibility_of(&edges, [10_000.0, 0.0, 0.0], [10_000.0, 0.0, 1_500.0],),
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
        let mut overlay = Overlay::with_shapes(
            &to_int_shapes(bounds.clone()),
            &to_int_shapes(contacts.clone()),
        );
        let oxide = from_int_shapes(overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd));
        let volumes = vec![
            WireVolume {
                shapes: bounds,
                bottom: 0,
                top: 40_000,
                material: 0,
                top_bevel: 0,
                bottom_bevel: 0,
            },
            WireVolume {
                shapes: oxide,
                bottom: 40_000,
                top: 45_000,
                material: 1,
                top_bevel: 0,
                bottom_bevel: 0,
            },
            WireVolume {
                shapes: contacts,
                bottom: 40_000,
                top: 50_000,
                material: 2,
                top_bevel: 0,
                bottom_bevel: 0,
            },
        ];
        let view = cetz_ortho_view(35.0_f64.to_radians(), 35.0_f64.to_radians());
        let surfaces = scene_surfaces(&volumes, view, [0.0, 0.0, 1.0], false, false);
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
    fn splits_a_receiver_into_lit_and_shadowed_regions() {
        let horizontal = |height, material, x0, x1| Face {
            normal: [0, 0, 1],
            material,
            interior: false,
            contours: vec![vec![
                [x0, 0, height],
                [x1, 0, height],
                [x1, 10, height],
                [x0, 10, height],
            ]],
        };
        let faces = vec![horizontal(0, 0, 0, 10), horizontal(10, 1, 0, 5)];
        let view = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let projected: Vec<ProjectedFace> = faces
            .iter()
            .map(|face| ProjectedFace::new(face, view))
            .collect();
        let grid = SurfaceGrid::new(&projected);
        let visible = projected_to_int_shapes(&projected[0].contours, grid);
        let regions = split_by_light_visibility(0, &faces, &visible, [0.0, 0.0, 1.0], view, grid);
        let lit = &regions
            .iter()
            .find(|(visibility, _)| *visibility == 1.0)
            .unwrap()
            .1;
        let shadowed = &regions
            .iter()
            .find(|(visibility, _)| *visibility == 0.0)
            .unwrap()
            .1;
        assert_eq!(shape_area2(lit), shape_area2(shadowed));
    }

    #[test]
    fn rejects_contours_that_collapse_in_projection() {
        let grid = SurfaceGrid {
            origin: [0.0, 0.0],
            scale: SURFACE_SCALE,
        };
        assert!(
            projected_to_int_shapes(&[vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0],]], grid).is_empty()
        );
        assert!(
            projected_to_int_shapes(
                &[vec![[0.0, 0.0], [0.000_1, 0.000_1], [0.000_2, 0.000_2],]],
                grid
            )
            .is_empty()
        );
        assert_eq!(
            projected_to_int_shapes(
                &[vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],]],
                grid
            )
            .len(),
            1,
        );
    }

    #[test]
    fn adapts_the_surface_grid_to_large_scenes() {
        let face = ProjectedFace {
            contours: Vec::new(),
            normal: [0.0, 0.0, 1.0],
            constant: 0.0,
            low: [-10_000_000.0, -5_000_000.0],
            high: [10_000_000.0, 5_000_000.0],
            material: 0,
        };
        let grid = SurfaceGrid::new(&[face]);
        assert!(grid.scale < SURFACE_SCALE);
        let encoded = grid.encode([10_000_000.0, 5_000_000.0]);
        assert!(f64::from(encoded.x) <= SURFACE_MAX_COORDINATE + 1.0);
        assert!(f64::from(encoded.y) <= SURFACE_MAX_COORDINATE + 1.0);
        let decoded = grid.decode(encoded);
        assert!((decoded[0] - 10_000_000.0).abs() <= 0.5 / grid.scale);
        assert!((decoded[1] - 5_000_000.0).abs() <= 0.5 / grid.scale);
    }

    fn projected_surface_shapes(
        surfaces: &[WireSurface],
        view: ViewMatrix,
        include: impl Fn(&WireSurface) -> bool,
    ) -> SurfaceShapes {
        let mut result = Vec::new();
        let grid = SurfaceGrid {
            origin: [0.0, 0.0],
            scale: SURFACE_SCALE,
        };
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
            let shapes = projected_to_int_shapes(&contours, grid);
            if result.is_empty() {
                result = shapes;
            } else {
                let mut overlay = Overlay::with_shapes(&result, &shapes);
                result = overlay.overlay(OverlayRule::Union, FillRule::EvenOdd);
            }
        }
        result
    }

    fn shape_area2(shapes: &SurfaceShapes) -> i128 {
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
