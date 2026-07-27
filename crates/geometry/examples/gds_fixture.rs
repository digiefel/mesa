use std::{env, fs::File};

use gds21::{GdsBoundary, GdsLibrary, GdsPoint, GdsStruct, GdsUnits};

fn boundary(layer: i16, points: &[(i32, i32)]) -> GdsBoundary {
    GdsBoundary {
        layer,
        datatype: 0,
        xy: GdsPoint::vec(points),
        ..Default::default()
    }
}

fn main() {
    let path = env::args()
        .nth(1)
        .expect("usage: cargo run --example gds_fixture -- OUTPUT.gds");

    let mut library = GdsLibrary::new("planar-mosfet");
    // This fixture's coordinates and layer thicknesses are expressed in nm.
    library.units = GdsUnits::new(1.0, 1e-9);
    let mut top = GdsStruct::new("TOP");
    top.elems.push(
        boundary(
            1,
            &[(0, 25), (120, 25), (120, 75), (0, 75), (0, 25)],
        )
        .into(),
    );
    top.elems.push(
        boundary(
            10,
            &[(55, 0), (65, 0), (65, 100), (55, 100), (55, 0)],
        )
        .into(),
    );
    top.elems.push(
        boundary(
            20,
            &[(15, 35), (45, 35), (45, 65), (15, 65), (15, 35)],
        )
        .into(),
    );
    top.elems.push(
        boundary(
            20,
            &[(75, 35), (105, 35), (105, 65), (75, 65), (75, 35)],
        )
        .into(),
    );
    library.structs.push(top);

    library
        .write(File::create(path).expect("could not create fixture"))
        .expect("could not write fixture");
}
