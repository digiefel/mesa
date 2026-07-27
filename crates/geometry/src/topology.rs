use std::collections::{BTreeMap, BTreeSet};

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay::Overlay;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::i_shape::int::shape::IntShapes;
use i_overlay::mesh::outline::offset::OutlineOffset;
use i_overlay::mesh::style::{LineJoin, OutlineStyle};
use serde::Deserialize;

use crate::{WireShapes, from_int_shapes, to_int_shapes};

pub(crate) type Point3 = [i64; 3];

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct WireVolume {
    pub(crate) shapes: WireShapes,
    pub(crate) bottom: i64,
    pub(crate) top: i64,
    pub(crate) material: u32,
    #[serde(default, rename = "top-bevel")]
    pub(crate) top_bevel: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum EdgeKind {
    Boundary,
    Crease,
    Bevel,
    Material,
    Smooth,
}

#[derive(Clone, Debug)]
struct TopBevel {
    material: u32,
    top: i64,
    shoulder: i64,
    interior: bool,
    original: Vec<[i64; 2]>,
    inset: Vec<[i64; 2]>,
    lines: BTreeSet<LineKey>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct VerticalFaceKey {
    bottom: i64,
    top: i64,
    material: u32,
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
        if volume.top_bevel < 0 || volume.top_bevel >= volume.top - volume.bottom {
            return Err(format!(
                "volume {index} requires 0 <= top_bevel < thickness; got {} for thickness {}",
                volume.top_bevel,
                volume.top - volume.bottom,
            ));
        }
        crate::validate_shapes(&format!("volume {index}"), &volume.shapes)?;
    }
    Ok(())
}

pub(crate) fn scene_geometry_with_smooth_join_cosine(
    volumes: &[WireVolume],
    smooth_join_cosine: f64,
) -> (Vec<Face>, Vec<AtomicEdge>) {
    let faces = scene_faces(volumes);
    let edges = atomic_edges(&faces, smooth_join_cosine);
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
    let mut vertical_faces = Vec::new();
    add_exposed_vertical_faces(&mut vertical_faces, &canonical);
    let bevels = top_bevels(&canonical, &vertical_faces);
    shorten_vertical_faces(&mut vertical_faces, &bevels);
    let mut faces = Vec::new();

    for (index, volume) in canonical.iter().enumerate() {
        let top_shapes = inset_beveled_contours(&volume.shapes, volume.material, &bevels);
        let top_cover = union_shapes(
            canonical
                .iter()
                .enumerate()
                .filter(|(other_index, other)| *other_index != index && other.bottom == volume.top)
                .map(|(_, other)| &other.shapes),
        );
        let exposed_top = difference_shapes(&top_shapes, &top_cover);
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

    add_top_bevel_faces(&mut faces, &bevels);
    faces.extend(vertical_faces);
    faces
}

fn top_bevels(volumes: &[WireVolume], vertical_faces: &[Face]) -> Vec<TopBevel> {
    let mut result = Vec::new();
    for volume in volumes {
        if volume.top_bevel == 0 {
            continue;
        }
        let shoulder = volume.top - volume.top_bevel;
        let exposed = exposed_top_lines(volume, shoulder, vertical_faces);
        for shape in &volume.shapes {
            for (contour_index, contour) in shape.iter().enumerate() {
                let lines: BTreeSet<LineKey> = contour
                    .iter()
                    .zip(contour.iter().cycle().skip(1))
                    .filter_map(|(&start, &end)| xy_line_coordinates(start, end))
                    .map(|coordinates| coordinates.key)
                    .collect();
                if lines.len() != contour.len() || !contour_top_is_exposed(contour, &exposed) {
                    continue;
                }
                let Some(inset) = inset_contour(contour, volume.top_bevel) else {
                    continue;
                };
                result.push(TopBevel {
                    material: volume.material,
                    top: volume.top,
                    shoulder,
                    interior: contour_index > 0,
                    original: contour.clone(),
                    inset,
                    lines,
                });
            }
        }
    }
    result
}

fn exposed_top_lines(
    volume: &WireVolume,
    shoulder: i64,
    vertical_faces: &[Face],
) -> BTreeMap<LineKey, Vec<(i128, i128)>> {
    let mut result: BTreeMap<LineKey, Vec<(i128, i128)>> = BTreeMap::new();
    for face in vertical_faces
        .iter()
        .filter(|face| face.material == volume.material)
    {
        let Some(contour) = face.contours.first() else {
            continue;
        };
        let bottom = contour.iter().map(|point| point[2]).min().unwrap_or(0);
        let top = contour.iter().map(|point| point[2]).max().unwrap_or(0);
        if top != volume.top || bottom > shoulder || contour.len() < 2 {
            continue;
        }
        let Some(coordinates) = xy_line_coordinates(
            [contour[0][0], contour[0][1]],
            [contour[1][0], contour[1][1]],
        ) else {
            continue;
        };
        result.entry(coordinates.key).or_default().push((
            coordinates.start.0.min(coordinates.end.0),
            coordinates.start.0.max(coordinates.end.0),
        ));
    }
    for spans in result.values_mut() {
        spans.sort_unstable();
    }
    result
}

fn contour_top_is_exposed(
    contour: &[[i64; 2]],
    exposed: &BTreeMap<LineKey, Vec<(i128, i128)>>,
) -> bool {
    contour
        .iter()
        .zip(contour.iter().cycle().skip(1))
        .all(|(&start, &end)| {
            let Some(coordinates) = xy_line_coordinates(start, end) else {
                return false;
            };
            let start = coordinates.start.0.min(coordinates.end.0);
            let end = coordinates.start.0.max(coordinates.end.0);
            let Some(spans) = exposed.get(&coordinates.key) else {
                return false;
            };
            let mut cursor = start;
            for &(span_start, span_end) in spans {
                if span_end <= cursor || end <= span_start {
                    continue;
                }
                if cursor < span_start {
                    return false;
                }
                cursor = cursor.max(span_end);
                if cursor >= end {
                    return true;
                }
            }
            false
        })
}

fn inset_contour(contour: &[[i64; 2]], distance: i64) -> Option<Vec<[i64; 2]>> {
    let sign = signed_area(contour).signum();
    let mut path: Vec<[f64; 2]> = contour
        .iter()
        .map(|point| [point[0] as f64, point[1] as f64])
        .collect();
    if sign < 0 {
        path.reverse();
    }
    let style = OutlineStyle::new(if sign < 0 {
        distance as f64
    } else {
        -(distance as f64)
    })
    .line_join(LineJoin::Miter(0.01));
    let shapes = path.outline_as::<i64>(&style);
    let expected = miter_inset(contour, distance)?;
    let expected_sign = sign as f64;
    let mut candidates = shapes
        .into_iter()
        .flatten()
        .map(|mut candidate| {
            if sign < 0 {
                candidate.reverse();
            }
            candidate
        })
        .filter(|candidate| {
            candidate.len() == contour.len()
                && float_signed_area(candidate).signum() == expected_sign
        });
    let candidate = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    align_contour(
        candidate
            .into_iter()
            .map(|point| [point[0].round() as i64, point[1].round() as i64])
            .collect(),
        &expected,
    )
}

fn miter_inset(contour: &[[i64; 2]], distance: i64) -> Option<Vec<[i64; 2]>> {
    let mut result = Vec::with_capacity(contour.len());
    for index in 0..contour.len() {
        let previous = contour[(index + contour.len() - 1) % contour.len()];
        let current = contour[index];
        let next = contour[(index + 1) % contour.len()];
        let first = offset_line(previous, current, distance)?;
        let second = offset_line(current, next, distance)?;
        let point = line_intersection(first, second)?;
        result.push([point[0].round() as i64, point[1].round() as i64]);
    }
    (signed_area(&result).signum() == signed_area(contour).signum()).then_some(result)
}

fn offset_line(start: [i64; 2], end: [i64; 2], distance: i64) -> Option<([f64; 2], [f64; 2])> {
    let direction = [(end[0] - start[0]) as f64, (end[1] - start[1]) as f64];
    let length = direction[0].hypot(direction[1]);
    if length == 0.0 {
        return None;
    }
    let normal = [-direction[1] / length, direction[0] / length];
    Some((
        [
            start[0] as f64 + normal[0] * distance as f64,
            start[1] as f64 + normal[1] * distance as f64,
        ],
        direction,
    ))
}

fn line_intersection(
    first: ([f64; 2], [f64; 2]),
    second: ([f64; 2], [f64; 2]),
) -> Option<[f64; 2]> {
    let denominator = first.1[0] * second.1[1] - first.1[1] * second.1[0];
    if denominator.abs() < 1e-9 {
        return None;
    }
    let offset = [second.0[0] - first.0[0], second.0[1] - first.0[1]];
    let distance = (offset[0] * second.1[1] - offset[1] * second.1[0]) / denominator;
    Some([
        first.0[0] + first.1[0] * distance,
        first.0[1] + first.1[1] * distance,
    ])
}

fn float_signed_area(contour: &[[f64; 2]]) -> f64 {
    contour
        .iter()
        .zip(contour.iter().cycle().skip(1))
        .map(|([x0, y0], [x1, y1])| x0 * y1 - y0 * x1)
        .sum()
}

fn align_contour(contour: Vec<[i64; 2]>, expected: &[[i64; 2]]) -> Option<Vec<[i64; 2]>> {
    let mut best = None;
    for reverse in [false, true] {
        let mut candidate = contour.clone();
        if reverse {
            candidate.reverse();
        }
        for shift in 0..candidate.len() {
            let aligned: Vec<[i64; 2]> = (0..candidate.len())
                .map(|index| candidate[(index + shift) % candidate.len()])
                .collect();
            let error = aligned
                .iter()
                .zip(expected)
                .map(|(point, target)| {
                    let dx = i128::from(point[0] - target[0]);
                    let dy = i128::from(point[1] - target[1]);
                    dx * dx + dy * dy
                })
                .sum::<i128>();
            if best
                .as_ref()
                .is_none_or(|(best_error, _)| error < *best_error)
            {
                best = Some((error, aligned));
            }
        }
    }
    best.map(|(_, contour)| contour)
}

fn inset_beveled_contours(shapes: &WireShapes, material: u32, bevels: &[TopBevel]) -> WireShapes {
    shapes
        .iter()
        .map(|shape| {
            shape
                .iter()
                .map(|contour| {
                    bevels
                        .iter()
                        .find(|bevel| bevel.material == material && bevel.original == *contour)
                        .map_or_else(|| contour.clone(), |bevel| bevel.inset.clone())
                })
                .collect()
        })
        .collect()
}

fn add_top_bevel_faces(faces: &mut Vec<Face>, bevels: &[TopBevel]) {
    for bevel in bevels {
        for index in 0..bevel.original.len() {
            let next = (index + 1) % bevel.original.len();
            let [x0, y0] = bevel.original[index];
            let [x1, y1] = bevel.original[next];
            let [ix0, iy0] = bevel.inset[index];
            let [ix1, iy1] = bevel.inset[next];
            let points = vec![
                [x0, y0, bevel.shoulder],
                [x1, y1, bevel.shoulder],
                [ix1, iy1, bevel.top],
                [ix0, iy0, bevel.top],
            ];
            let normal = cross_wide(
                [
                    points[1][0] - points[0][0],
                    points[1][1] - points[0][1],
                    points[1][2] - points[0][2],
                ],
                [
                    points[2][0] - points[0][0],
                    points[2][1] - points[0][1],
                    points[2][2] - points[0][2],
                ],
            );
            let divisor = gcd(gcd(normal[0] as i64, normal[1] as i64), normal[2] as i64);
            if divisor == 0 {
                continue;
            }
            faces.push(Face {
                normal: [
                    normal[0] as i64 / divisor,
                    normal[1] as i64 / divisor,
                    normal[2] as i64 / divisor,
                ],
                material: bevel.material,
                interior: bevel.interior,
                contours: vec![points],
            });
        }
    }
}

fn shorten_vertical_faces(faces: &mut Vec<Face>, bevels: &[TopBevel]) {
    for face in faces.iter_mut() {
        let Some(bevel) = bevels.iter().find(|bevel| {
            if bevel.material != face.material {
                return false;
            }
            let Some(contour) = face.contours.first() else {
                return false;
            };
            if contour.len() < 2 {
                return false;
            }
            let Some(coordinates) = xy_line_coordinates(
                [contour[0][0], contour[0][1]],
                [contour[1][0], contour[1][1]],
            ) else {
                return false;
            };
            bevel.lines.contains(&coordinates.key)
                && contour.iter().any(|point| point[2] == bevel.top)
        }) else {
            continue;
        };
        for contour in &mut face.contours {
            for point in contour {
                if point[2] == bevel.top {
                    point[2] = bevel.shoulder;
                }
            }
        }
    }
    faces.retain(|face| {
        face.contours.iter().flatten().map(|point| point[2]).min()
            != face.contours.iter().flatten().map(|point| point[2]).max()
            || face.normal[2] != 0
    });
}

fn xy_line_coordinates(start: [i64; 2], end: [i64; 2]) -> Option<LineCoordinates> {
    line_coordinates(Segment {
        start: [start[0], start[1], 0],
        end: [end[0], end[1], 0],
        interior: false,
    })
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
        let mut exposed: BTreeMap<VerticalFaceKey, Vec<(i128, i128)>> = BTreeMap::new();
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
                    exposed
                        .entry(VerticalFaceKey {
                            bottom,
                            top,
                            material: volume.material,
                            normal: interval.normal,
                            interior: interval.interior,
                        })
                        .or_default()
                        .push((start, end));
                }
            }
        }

        for (key, mut spans) in exposed {
            spans.sort_unstable();
            let mut merged: Vec<(i128, i128)> = Vec::new();
            for (start, end) in spans {
                if let Some(last) = merged.last_mut()
                    && last.1 == start
                {
                    last.1 = end;
                } else {
                    merged.push((start, end));
                }
            }
            for (start, end) in merged {
                let [x0, y0, _] = line.points[&start];
                let [x1, y1, _] = line.points[&end];
                faces.push(Face {
                    normal: key.normal,
                    material: key.material,
                    interior: key.interior,
                    contours: vec![vec![
                        [x0, y0, key.bottom],
                        [x1, y1, key.bottom],
                        [x1, y1, key.top],
                        [x0, y0, key.top],
                    ]],
                });
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

fn atomic_edges(faces: &[Face], smooth_join_cosine: f64) -> Vec<AtomicEdge> {
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
            let kind = classify_edge(faces, &incident, smooth_join_cosine);
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
    face.contours
        .iter()
        .enumerate()
        .flat_map(move |(contour_index, contour)| {
            (0..contour.len()).map(move |index| Segment {
                start: contour[index],
                end: contour[(index + 1) % contour.len()],
                interior: face_interior || contour_index > 0,
            })
        })
}

fn classify_edge(
    faces: &[Face],
    incident: &BTreeSet<usize>,
    smooth_join_cosine: f64,
) -> EdgeKind {
    if incident.len() == 1 {
        return EdgeKind::Boundary;
    }

    let incident: Vec<&Face> = incident.iter().map(|index| &faces[*index]).collect();
    if incident
        .iter()
        .map(|face| face.material)
        .collect::<BTreeSet<_>>()
        .len()
        > 1
    {
        return EdgeKind::Material;
    }

    let is_bevel =
        |face: &&Face| face.normal[2] != 0 && (face.normal[0] != 0 || face.normal[1] != 0);
    if incident.iter().any(is_bevel) && incident.iter().any(|face| face.normal[2] == 0) {
        return EdgeKind::Bevel;
    }

    for (index, face) in incident.iter().enumerate() {
        for other in incident.iter().skip(index + 1) {
            if cross_wide(face.normal, other.normal) != [0, 0, 0]
                && !normals_form_smooth_join(
                    face.normal,
                    other.normal,
                    smooth_join_cosine,
                )
            {
                return EdgeKind::Crease;
            }
        }
    }

    EdgeKind::Smooth
}

fn normals_form_smooth_join(left: Point3, right: Point3, minimum_cosine: f64) -> bool {
    let dot = dot_wide(left, right) as f64;
    if dot <= 0.0 {
        return false;
    }
    let left_length = (dot_wide(left, left) as f64).sqrt();
    let right_length = (dot_wide(right, right) as f64).sqrt();
    dot >= minimum_cosine * left_length * right_length
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
    fn decodes_the_typst_top_bevel_wire_field() {
        #[derive(serde::Serialize)]
        struct VolumePayload<'a> {
            shapes: &'a WireShapes,
            bottom: i64,
            top: i64,
            material: u32,
            #[serde(rename = "top-bevel")]
            top_bevel: i64,
        }

        let shapes = vec![vec![rectangle(0, 0, 10_000, 8_000)]];
        let mut bytes = Vec::new();
        ciborium::into_writer(
            &VolumePayload {
                shapes: &shapes,
                bottom: 0,
                top: 5_000,
                material: 0,
                top_bevel: 500,
            },
            &mut bytes,
        )
        .unwrap();
        let volume: WireVolume = ciborium::from_reader(bytes.as_slice()).unwrap();

        assert_eq!(volume.top_bevel, 500);
    }

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

        let edges = atomic_edges(&faces, 1.0);
        assert_eq!(edges.len(), 3);
        assert_eq!(edges[0].kind, EdgeKind::Material);
        assert_eq!(edges[1].kind, EdgeKind::Crease);
        assert_eq!(edges[2].kind, EdgeKind::Material);
        assert!(edges.iter().all(|edge| edge.incident.len() == 2));
    }

    #[test]
    fn treats_shallow_polygon_joins_as_smooth_curves() {
        let face = |normal| Face {
            normal,
            material: 0,
            interior: false,
            contours: Vec::new(),
        };
        let incident = BTreeSet::from([0, 1]);

        assert_eq!(
            classify_edge(
                &[face([0, -1000, 0]), face([342, -940, 0])],
                &incident,
                1.0,
            ),
            EdgeKind::Crease,
        );
        assert_eq!(
            classify_edge(
                &[face([0, -1000, 0]), face([342, -940, 0])],
                &incident,
                30.0_f64.to_radians().cos(),
            ),
            EdgeKind::Smooth,
        );
        assert_eq!(
            classify_edge(
                &[face([0, -1000, 0]), face([707, -707, 0])],
                &incident,
                30.0_f64.to_radians().cos(),
            ),
            EdgeKind::Crease,
        );
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
                top_bevel: 0,
            },
            WireVolume {
                shapes: result,
                bottom: 0,
                top: 1500,
                material: 1,
                top_bevel: 0,
            },
        ];

        let (_, edges) = scene_geometry_with_smooth_join_cosine(&volumes, 1.0);
        assert_eq!(
            edge_kind(&edges, [0, 0, -1500], [0, 0, 0]),
            Some(EdgeKind::Crease)
        );
        assert_eq!(
            edge_kind(&edges, [2, 1, 0], [2, 1, 1500]),
            Some(EdgeKind::Crease)
        );
        assert_eq!(edge_interior(&edges, [0, 0, -1500], [0, 0, 0]), Some(false));
        assert_eq!(edge_interior(&edges, [2, 1, 0], [2, 1, 1500]), Some(true));
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
                top_bevel: 0,
            },
            WireVolume {
                shapes: oxide,
                bottom: 40,
                top: 45,
                material: 1,
                top_bevel: 0,
            },
            WireVolume {
                shapes: contact,
                bottom: 40,
                top: 50,
                material: 2,
                top_bevel: 0,
            },
        ];

        let faces = scene_faces(&volumes);
        assert!(
            !faces
                .iter()
                .any(|face| face.material == 0 && face.normal == [0, 0, 1])
        );
        assert!(
            faces
                .iter()
                .filter(|face| face.material == 2 && face.normal[2] == 0)
                .flat_map(|face| face.contours.iter().flatten())
                .all(|point| point[2] >= 45)
        );
    }

    #[test]
    fn does_not_split_a_continuous_face_at_an_upper_layer_endpoint() {
        let bounds = vec![vec![rectangle(0, 0, 10, 6)]];
        let upper = vec![vec![rectangle(4, 0, 6, 6)]];
        let volumes = vec![
            WireVolume {
                shapes: bounds,
                bottom: 0,
                top: 40,
                material: 0,
                top_bevel: 0,
            },
            WireVolume {
                shapes: upper,
                bottom: 40,
                top: 50,
                material: 1,
                top_bevel: 0,
            },
        ];

        let faces = scene_faces(&volumes);
        let substrate_front: Vec<&Face> = faces
            .iter()
            .filter(|face| face.material == 0 && face.normal == [0, -10, 0])
            .collect();
        assert_eq!(substrate_front.len(), 1);
        assert_eq!(
            substrate_front[0].contours,
            vec![vec![[0, 0, 0], [10, 0, 0], [10, 0, 40], [0, 0, 40]]]
        );
    }

    #[test]
    fn builds_a_real_top_bevel_for_a_polygon_volume() {
        let volumes = vec![WireVolume {
            shapes: vec![vec![rectangle(0, 0, 10_000, 8_000)]],
            bottom: 0,
            top: 5_000,
            material: 0,
            top_bevel: 1_000,
        }];

        let (faces, edges) = scene_geometry_with_smooth_join_cosine(&volumes, 1.0);
        let bevel_faces: Vec<&Face> = faces
            .iter()
            .filter(|face| face.normal[2] != 0 && (face.normal[0] != 0 || face.normal[1] != 0))
            .collect();
        assert_eq!(bevel_faces.len(), 4);
        assert!(bevel_faces.iter().all(|face| {
            face.contours.iter().flatten().map(|point| point[2]).min() == Some(4_000)
        }));

        let top = faces.iter().find(|face| face.normal == [0, 0, 1]).unwrap();
        let top_points: BTreeSet<Point3> = top.contours.iter().flatten().copied().collect();
        assert_eq!(
            top_points,
            BTreeSet::from([
                [1_000, 1_000, 5_000],
                [9_000, 1_000, 5_000],
                [9_000, 7_000, 5_000],
                [1_000, 7_000, 5_000],
            ])
        );
        assert_eq!(
            edge_kind(&edges, [0, 0, 4_000], [10_000, 0, 4_000]),
            Some(EdgeKind::Bevel)
        );
        assert_eq!(
            edge_kind(&edges, [1_000, 1_000, 5_000], [9_000, 1_000, 5_000]),
            Some(EdgeKind::Crease)
        );
    }

    #[test]
    fn bevels_an_exposed_hole_but_not_a_buried_material_interface() {
        let outer = rectangle(0, 0, 10_000, 10_000);
        let hole: Vec<[i64; 2]> = rectangle(3_000, 3_000, 7_000, 7_000)
            .into_iter()
            .rev()
            .collect();
        assert!(inset_contour(&hole, 500).is_some());
        let void_faces = scene_faces(&[WireVolume {
            shapes: vec![vec![outer.clone(), hole.clone()]],
            bottom: 0,
            top: 2_000,
            material: 0,
            top_bevel: 500,
        }]);
        assert_eq!(
            void_faces
                .iter()
                .filter(|face| {
                    face.normal[2] != 0 && (face.normal[0] != 0 || face.normal[1] != 0)
                })
                .count(),
            8
        );

        let material_faces = scene_faces(&[
            WireVolume {
                shapes: vec![vec![outer, hole]],
                bottom: 0,
                top: 1_000,
                material: 0,
                top_bevel: 500,
            },
            WireVolume {
                shapes: vec![vec![rectangle(3_000, 3_000, 7_000, 7_000)]],
                bottom: 0,
                top: 2_000,
                material: 1,
                top_bevel: 500,
            },
        ]);
        assert_eq!(
            material_faces
                .iter()
                .filter(|face| {
                    face.material == 0
                        && face.normal[2] != 0
                        && (face.normal[0] != 0 || face.normal[1] != 0)
                })
                .count(),
            4
        );
        assert_eq!(
            material_faces
                .iter()
                .filter(|face| {
                    face.material == 1
                        && face.normal[2] != 0
                        && (face.normal[0] != 0 || face.normal[1] != 0)
                })
                .count(),
            4
        );
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
