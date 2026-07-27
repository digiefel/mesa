use std::collections::{BTreeMap, BTreeSet};

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::Overlay;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_shape::int::shape::IntShapes;
use serde::Deserialize;

use crate::{WireShapes, from_int_shapes, to_int_shapes};

pub(crate) type Point3 = [i64; 3];

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct WireVolume {
    pub(crate) shapes: WireShapes,
    pub(crate) bottom: i64,
    pub(crate) top: i64,
    pub(crate) material: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum EdgeKind {
    Boundary,
    Crease,
    Material,
    Smooth,
}

#[derive(Clone, Debug)]
pub(crate) struct AtomicEdge {
    pub(crate) start: Point3,
    pub(crate) end: Point3,
    pub(crate) kind: EdgeKind,
    pub(crate) interior: bool,
    pub(crate) incident: BTreeSet<usize>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct Face {
    pub(crate) normal: Point3,
    pub(crate) material: u32,
    interior: bool,
    pub(crate) contours: Vec<Vec<Point3>>,
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    start: Point3,
    end: Point3,
    interior: bool,
}

#[derive(Clone, Copy, Debug)]
struct FaceInterval {
    start: i128,
    end: i128,
    face: usize,
    interior: bool,
}

#[derive(Clone, Copy, Debug)]
struct VerticalFaceInterval {
    start: i128,
    end: i128,
    volume: usize,
    normal: Point3,
    interior: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LineKey {
    direction: Point3,
    moment: [i128; 3],
}

#[derive(Default)]
struct LineSegments {
    points: BTreeMap<i128, Point3>,
    intervals: Vec<FaceInterval>,
}

#[derive(Default)]
struct VerticalLineSegments {
    points: BTreeMap<i128, Point3>,
    intervals: Vec<VerticalFaceInterval>,
}

struct LineCoordinates {
    key: LineKey,
    start: (i128, Point3),
    end: (i128, Point3),
}

pub(crate) fn validate_volumes(volumes: &[WireVolume]) -> Result<(), String> {
    for (index, volume) in volumes.iter().enumerate() {
        if volume.top <= volume.bottom {
            return Err(format!(
                "volume {index} requires top > bottom; got {} <= {}",
                volume.top, volume.bottom
            ));
        }
        crate::validate_shapes(&format!("volume {index}"), &volume.shapes)?;
    }
    Ok(())
}

pub(crate) fn scene_geometry(volumes: &[WireVolume]) -> (Vec<Face>, Vec<AtomicEdge>) {
    let faces = scene_faces(volumes);
    let edges = atomic_edges(&faces);
    (faces, edges)
}

pub(crate) fn scene_faces(volumes: &[WireVolume]) -> Vec<Face> {
    let canonical: Vec<WireVolume> = volumes
        .iter()
        .map(|volume| WireVolume {
            shapes: normalize_shapes(volume.shapes.clone()),
            ..volume.clone()
        })
        .collect();
    let mut faces = Vec::new();

    for (index, volume) in canonical.iter().enumerate() {
        let top_cover = union_shapes(
            canonical
                .iter()
                .enumerate()
                .filter(|(other_index, other)| *other_index != index && other.bottom == volume.top)
                .map(|(_, other)| &other.shapes),
        );
        let exposed_top = difference_shapes(&volume.shapes, &top_cover);
        add_horizontal_faces(
            &mut faces,
            &exposed_top,
            volume.top,
            [0, 0, 1],
            volume.material,
        );

        let bottom_cover = union_shapes(
            canonical
                .iter()
                .enumerate()
                .filter(|(other_index, other)| *other_index != index && other.top == volume.bottom)
                .map(|(_, other)| &other.shapes),
        );
        let exposed_bottom = difference_shapes(&volume.shapes, &bottom_cover);
        add_horizontal_faces(
            &mut faces,
            &exposed_bottom,
            volume.bottom,
            [0, 0, -1],
            volume.material,
        );

    }

    add_exposed_vertical_faces(&mut faces, &canonical);
    faces
}

fn add_horizontal_faces(
    faces: &mut Vec<Face>,
    shapes: &WireShapes,
    height: i64,
    normal: Point3,
    material: u32,
) {
    for shape in shapes {
        let contours = shape
            .iter()
            .map(|contour| contour.iter().map(|[x, y]| [*x, *y, height]).collect())
            .collect();
        faces.push(Face {
            normal,
            material,
            interior: false,
            contours,
        });
    }
}

fn add_exposed_vertical_faces(faces: &mut Vec<Face>, volumes: &[WireVolume]) {
    let mut lines: BTreeMap<LineKey, VerticalLineSegments> = BTreeMap::new();
    for (volume, geometry) in volumes.iter().enumerate() {
        for shape in &geometry.shapes {
            for (contour_index, contour) in shape.iter().enumerate() {
                for index in 0..contour.len() {
                    let [x0, y0] = contour[index];
                    let [x1, y1] = contour[(index + 1) % contour.len()];
                    let Some(coordinates) = line_coordinates(Segment {
                        start: [x0, y0, 0],
                        end: [x1, y1, 0],
                        interior: contour_index > 0,
                    }) else {
                        continue;
                    };
                    let line = lines.entry(coordinates.key).or_default();
                    line.points.insert(coordinates.start.0, coordinates.start.1);
                    line.points.insert(coordinates.end.0, coordinates.end.1);
                    line.intervals.push(VerticalFaceInterval {
                        start: coordinates.start.0.min(coordinates.end.0),
                        end: coordinates.start.0.max(coordinates.end.0),
                        volume,
                        normal: [y1 - y0, x0 - x1, 0],
                        interior: contour_index > 0,
                    });
                }
            }
        }
    }

    for line in lines.values() {
        let coordinates: Vec<i128> = line.points.keys().copied().collect();
        for pair in coordinates.windows(2) {
            let start = pair[0];
            let end = pair[1];
            let incident: Vec<VerticalFaceInterval> = line
                .intervals
                .iter()
                .filter(|interval| interval.start <= start && end <= interval.end)
                .copied()
                .collect();
            let mut rendered = BTreeSet::new();
            for interval in &incident {
                if !rendered.insert((interval.volume, interval.normal, interval.interior)) {
                    continue;
                }
                let volume = &volumes[interval.volume];
                let covered = incident
                    .iter()
                    .filter(|other| other.volume != interval.volume)
                    .map(|other| {
                        let other = &volumes[other.volume];
                        (other.bottom, other.top)
                    });
                for (bottom, top) in uncovered_intervals(volume.bottom, volume.top, covered) {
                    let [x0, y0, _] = line.points[&start];
                    let [x1, y1, _] = line.points[&end];
                    faces.push(Face {
                        normal: interval.normal,
                        material: volume.material,
                        interior: interval.interior,
                        contours: vec![vec![
                            [x0, y0, bottom],
                            [x1, y1, bottom],
                            [x1, y1, top],
                            [x0, y0, top],
                        ]],
                    });
                }
            }
        }
    }
}

fn uncovered_intervals(
    bottom: i64,
    top: i64,
    covered: impl Iterator<Item = (i64, i64)>,
) -> Vec<(i64, i64)> {
    let mut covered: Vec<(i64, i64)> = covered
        .filter_map(|(other_bottom, other_top)| {
            let start = bottom.max(other_bottom);
            let end = top.min(other_top);
            (start < end).then_some((start, end))
        })
        .collect();
    covered.sort_unstable();

    let mut result = Vec::new();
    let mut cursor = bottom;
    for (start, end) in covered {
        if cursor < start {
            result.push((cursor, start));
        }
        cursor = cursor.max(end);
        if cursor >= top {
            break;
        }
    }
    if cursor < top {
        result.push((cursor, top));
    }
    result
}

fn atomic_edges(faces: &[Face]) -> Vec<AtomicEdge> {
    let mut lines: BTreeMap<LineKey, LineSegments> = BTreeMap::new();

    for (face, surface) in faces.iter().enumerate() {
        for segment in face_segments(surface) {
            let Some(coordinates) = line_coordinates(segment) else {
                continue;
            };
            let line = lines.entry(coordinates.key).or_default();
            line.points.insert(coordinates.start.0, coordinates.start.1);
            line.points.insert(coordinates.end.0, coordinates.end.1);
            line.intervals.push(FaceInterval {
                start: coordinates.start.0.min(coordinates.end.0),
                end: coordinates.start.0.max(coordinates.end.0),
                face,
                interior: segment.interior,
            });
        }
    }

    let mut result = Vec::new();
    for line in lines.values() {
        let coordinates: Vec<i128> = line.points.keys().copied().collect();
        for pair in coordinates.windows(2) {
            let start = pair[0];
            let end = pair[1];
            let incident: BTreeSet<usize> = line
                .intervals
                .iter()
                .filter(|interval| interval.start <= start && end <= interval.end)
                .map(|interval| interval.face)
                .collect();
            if incident.is_empty() {
                continue;
            }
            let kind = classify_edge(faces, &incident);
            let interior = line.intervals.iter().any(|interval| {
                interval.start <= start && end <= interval.end && interval.interior
            });
            result.push(AtomicEdge {
                start: line.points[&start],
                end: line.points[&end],
                kind,
                interior,
                incident,
            });
        }
    }

    result
}

fn face_segments(face: &Face) -> impl Iterator<Item = Segment> + '_ {
    let face_interior = face.interior;
    face.contours.iter().enumerate().flat_map(move |(contour_index, contour)| {
        (0..contour.len()).map(move |index| Segment {
            start: contour[index],
            end: contour[(index + 1) % contour.len()],
            interior: face_interior || contour_index > 0,
        })
    })
}

fn classify_edge(faces: &[Face], incident: &BTreeSet<usize>) -> EdgeKind {
    if incident.len() == 1 {
        return EdgeKind::Boundary;
    }

    let incident: Vec<&Face> = incident.iter().map(|index| &faces[*index]).collect();
    for (index, face) in incident.iter().enumerate() {
        for other in incident.iter().skip(index + 1) {
            if cross_wide(face.normal, other.normal) != [0, 0, 0] {
                return EdgeKind::Crease;
            }
        }
    }

    if incident
        .iter()
        .map(|face| face.material)
        .collect::<BTreeSet<_>>()
        .len()
        > 1
    {
        EdgeKind::Material
    } else {
        EdgeKind::Smooth
    }
}

fn line_coordinates(segment: Segment) -> Option<LineCoordinates> {
    let mut direction = [
        segment.end[0] - segment.start[0],
        segment.end[1] - segment.start[1],
        segment.end[2] - segment.start[2],
    ];
    let divisor = gcd(gcd(direction[0], direction[1]), direction[2]);
    if divisor == 0 {
        return None;
    }
    for component in &mut direction {
        *component /= divisor;
    }
    if direction
        .iter()
        .find(|component| **component != 0)
        .is_some_and(|component| *component < 0)
    {
        for component in &mut direction {
            *component = -*component;
        }
    }

    let key = LineKey {
        direction,
        moment: cross_wide(segment.start, direction),
    };
    let start = dot_wide(segment.start, direction);
    let end = dot_wide(segment.end, direction);
    Some(LineCoordinates {
        key,
        start: (start, segment.start),
        end: (end, segment.end),
    })
}

fn normalize_shapes(shapes: WireShapes) -> WireShapes {
    shapes
        .into_iter()
        .map(|shape| {
            shape
                .into_iter()
                .enumerate()
                .map(|(index, mut contour)| {
                    let area = signed_area(&contour);
                    let should_be_positive = index == 0;
                    if (area > 0) != should_be_positive {
                        contour.reverse();
                    }
                    contour
                })
                .collect()
        })
        .collect()
}

fn union_shapes<'a>(shapes: impl Iterator<Item = &'a WireShapes>) -> WireShapes {
    let mut result: IntShapes<i64> = Vec::new();
    for shapes in shapes {
        let shapes = to_int_shapes(shapes.clone());
        if result.is_empty() {
            result = shapes;
        } else if !shapes.is_empty() {
            let mut overlay = Overlay::with_shapes(&result, &shapes);
            result = overlay.overlay(OverlayRule::Union, FillRule::EvenOdd);
        }
    }
    from_int_shapes(result)
}

fn difference_shapes(subject: &WireShapes, clip: &WireShapes) -> WireShapes {
    if clip.is_empty() {
        return subject.clone();
    }
    let subject = to_int_shapes(subject.clone());
    let clip = to_int_shapes(clip.clone());
    let mut overlay = Overlay::with_shapes(&subject, &clip);
    from_int_shapes(overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd))
}

fn signed_area(contour: &[[i64; 2]]) -> i128 {
    contour
        .iter()
        .zip(contour.iter().cycle().skip(1))
        .map(|([x0, y0], [x1, y1])| {
            i128::from(*x0) * i128::from(*y1) - i128::from(*y0) * i128::from(*x1)
        })
        .sum()
}

fn gcd(left: i64, right: i64) -> i64 {
    let mut left = left.unsigned_abs();
    let mut right = right.unsigned_abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left as i64
}

fn cross_wide(left: Point3, right: Point3) -> [i128; 3] {
    [
        i128::from(left[1]) * i128::from(right[2]) - i128::from(left[2]) * i128::from(right[1]),
        i128::from(left[2]) * i128::from(right[0]) - i128::from(left[0]) * i128::from(right[2]),
        i128::from(left[0]) * i128::from(right[1]) - i128::from(left[1]) * i128::from(right[0]),
    ]
}

fn dot_wide(left: Point3, right: Point3) -> i128 {
    i128::from(left[0]) * i128::from(right[0])
        + i128::from(left[1]) * i128::from(right[1])
        + i128::from(left[2]) * i128::from(right[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_overlapping_collinear_edges_before_classification() {
        let faces = vec![
            Face {
                normal: [0, -1, 0],
                material: 0,
                interior: false,
                contours: vec![vec![[0, 0, 0], [10, 0, 0]]],
            },
            Face {
                normal: [0, -1, 0],
                material: 1,
                interior: false,
                contours: vec![vec![[0, 0, 0], [6, 0, 0]]],
            },
            Face {
                normal: [0, 0, 1],
                material: 0,
                interior: false,
                contours: vec![vec![[6, 0, 0], [7, 0, 0]]],
            },
            Face {
                normal: [0, -1, 0],
                material: 1,
                interior: false,
                contours: vec![vec![[7, 0, 0], [10, 0, 0]]],
            },
        ];

        let edges = atomic_edges(&faces);
        assert_eq!(edges.len(), 3);
        assert_eq!(edges[0].kind, EdgeKind::Material);
        assert_eq!(edges[1].kind, EdgeKind::Crease);
        assert_eq!(edges[2].kind, EdgeKind::Material);
        assert!(edges.iter().all(|edge| edge.incident.len() == 2));
    }

    #[test]
    fn classifies_outer_and_inner_vertical_corners_as_creases() {
        let subject = vec![vec![rectangle(0, 0, 10, 6)]];
        let mask = vec![vec![rectangle(2, 1, 4, 3)], vec![rectangle(6, -1, 7, 7)]];
        let mut overlay =
            Overlay::with_shapes(&to_int_shapes(subject.clone()), &to_int_shapes(mask));
        let result = from_int_shapes(overlay.overlay(OverlayRule::Difference, FillRule::EvenOdd));
        let volumes = vec![
            WireVolume {
                shapes: subject,
                bottom: -1500,
                top: 0,
                material: 0,
            },
            WireVolume {
                shapes: result,
                bottom: 0,
                top: 1500,
                material: 1,
            },
        ];

        let (_, edges) = scene_geometry(&volumes);
        assert_eq!(
            edge_kind(&edges, [0, 0, -1500], [0, 0, 0]),
            Some(EdgeKind::Crease)
        );
        assert_eq!(
            edge_kind(&edges, [2, 1, 0], [2, 1, 1500]),
            Some(EdgeKind::Crease)
        );
        assert_eq!(
            edge_interior(&edges, [0, 0, -1500], [0, 0, 0]),
            Some(false)
        );
        assert_eq!(
            edge_interior(&edges, [2, 1, 0], [2, 1, 1500]),
            Some(true)
        );
        assert!(edges.iter().all(|edge| edge.kind != EdgeKind::Boundary));
    }

    #[test]
    fn removes_faces_buried_between_adjacent_materials() {
        let bounds = vec![vec![rectangle(0, 0, 10, 6)]];
        let contact = vec![vec![rectangle(2, 1, 4, 3)]];
        let oxide = difference_shapes(&bounds, &contact);
        let volumes = vec![
            WireVolume {
                shapes: bounds,
                bottom: 0,
                top: 40,
                material: 0,
            },
            WireVolume {
                shapes: oxide,
                bottom: 40,
                top: 45,
                material: 1,
            },
            WireVolume {
                shapes: contact,
                bottom: 40,
                top: 50,
                material: 2,
            },
        ];

        let faces = scene_faces(&volumes);
        assert!(!faces
            .iter()
            .any(|face| face.material == 0 && face.normal == [0, 0, 1]));
        assert!(faces
            .iter()
            .filter(|face| face.material == 2 && face.normal[2] == 0)
            .flat_map(|face| face.contours.iter().flatten())
            .all(|point| point[2] >= 45));
    }

    fn edge_kind(edges: &[AtomicEdge], start: Point3, end: Point3) -> Option<EdgeKind> {
        edges
            .iter()
            .find(|edge| {
                (edge.start == start && edge.end == end) || (edge.start == end && edge.end == start)
            })
            .map(|edge| edge.kind)
    }

    fn edge_interior(edges: &[AtomicEdge], start: Point3, end: Point3) -> Option<bool> {
        edges
            .iter()
            .find(|edge| {
                (edge.start == start && edge.end == end) || (edge.start == end && edge.end == start)
            })
            .map(|edge| edge.interior)
    }

    fn rectangle(left: i64, bottom: i64, right: i64, top: i64) -> Vec<[i64; 2]> {
        vec![[left, bottom], [right, bottom], [right, top], [left, top]]
    }
}
