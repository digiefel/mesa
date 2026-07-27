use std::collections::BTreeSet;

use gds21::{GdsElement, GdsLibrary};
use serde::Serialize;

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

#[cfg(test)]
mod tests {
    use gds21::{GdsBoundary, GdsLibrary, GdsPoint, GdsStruct};

    use super::*;

    #[test]
    fn reads_library_cells_units_and_boundary_layers_from_bytes() {
        let mut library = GdsLibrary::new("semi-example");
        let mut top = GdsStruct::new("TOP");
        top.elems.push(
            GdsBoundary {
                layer: 20,
                datatype: 0,
                xy: GdsPoint::vec(&[(0, 0), (8, 0), (8, 3), (0, 3), (0, 0)]),
                ..Default::default()
            }
            .into(),
        );
        top.elems.push(
            GdsBoundary {
                layer: 1,
                datatype: 0,
                xy: GdsPoint::vec(&[(1, 1), (7, 1), (7, 2), (1, 2), (1, 1)]),
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
    fn rejects_invalid_gds_bytes() {
        assert!(inspect(b"not a GDS file").is_err());
    }
}
