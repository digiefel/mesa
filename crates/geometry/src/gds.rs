use std::collections::{BTreeMap, BTreeSet};

use gds21::{GdsElement, GdsLibrary};
use serde::{Deserialize, Serialize};

type GdsPoint = [f64; 2];
type GdsContour = Vec<GdsPoint>;
type GdsShape = Vec<GdsContour>;
type GdsShapes = Vec<GdsShape>;

#[derive(Debug, Deserialize)]
pub(crate) struct GdsLayoutRequest {
    pub(crate) cell: String,
    pub(crate) layers: BTreeMap<String, [i16; 2]>,
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

    let mut layers = request
        .layers
        .keys()
        .map(|name| (name.clone(), GdsShapes::new()))
        .collect::<BTreeMap<_, _>>();
    let mut bounds: Option<[i32; 4]> = None;

    for element in &cell.elems {
        let GdsElement::GdsBoundary(boundary) = element else {
            continue;
        };
        let Some((name, _)) = request
            .layers
            .iter()
            .find(|(_, layer)| **layer == [boundary.layer, boundary.datatype])
        else {
            continue;
        };

        let mut contour = boundary
            .xy
            .iter()
            .map(|point| {
                bounds = Some(match bounds {
                    Some([min_x, min_y, max_x, max_y]) => [
                        min_x.min(point.x),
                        min_y.min(point.y),
                        max_x.max(point.x),
                        max_y.max(point.y),
                    ],
                    None => [point.x, point.y, point.x, point.y],
                });
                [point.x as f64, point.y as f64]
            })
            .collect::<GdsContour>();

        if contour.len() >= 2 && contour.first() == contour.last() {
            contour.pop();
        }
        if contour.len() >= 3 {
            layers.get_mut(name).unwrap().push(vec![contour]);
        }
    }

    let [min_x, min_y, max_x, max_y] =
        bounds.ok_or_else(|| "selected GDS layers contain no boundary polygons".to_string())?;
    let nanometers_per_database_unit = library.units.db_unit() / 1e-9;
    let origin = [
        min_x as f64 * nanometers_per_database_unit,
        min_y as f64 * nanometers_per_database_unit,
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
            (max_x - min_x) as f64 * nanometers_per_database_unit,
            (max_y - min_y) as f64 * nanometers_per_database_unit,
        ],
        layers,
    })
}

#[cfg(test)]
mod tests {
    use gds21::{
        GdsBoundary, GdsLibrary, GdsPoint as LibraryPoint, GdsStruct, GdsStructRef,
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
                layers: BTreeMap::from([
                    ("active".into(), [1, 0]),
                    ("gate".into(), [10, 2]),
                ]),
            },
        )
        .unwrap();

        assert_eq!(layout.origin, [100.0, 200.0]);
        assert_eq!(layout.size, [10.0, 6.0]);
        assert_eq!(
            layout.layers["gate"],
            vec![vec![vec![
                [2.0, 1.0],
                [8.0, 1.0],
                [8.0, 5.0],
                [2.0, 5.0],
            ]]]
        );
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
