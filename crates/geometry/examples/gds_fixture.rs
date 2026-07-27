use std::{env, fs::File};

use gds21::{GdsBoundary, GdsLibrary, GdsPoint, GdsStruct};

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

    let mut library = GdsLibrary::new("semi-example");
    let mut top = GdsStruct::new("TOP");
    top.elems
        .push(boundary(1, &[(0, 0), (100, 0), (100, 60), (0, 60), (0, 0)]).into());
    top.elems.push(
        boundary(
            10,
            &[(20, 10), (80, 10), (80, 50), (20, 50), (20, 10)],
        )
        .into(),
    );
    top.elems.push(
        boundary(
            20,
            &[(45, 0), (55, 0), (55, 60), (45, 60), (45, 0)],
        )
        .into(),
    );
    library.structs.push(top);

    library
        .write(File::create(path).expect("could not create fixture"))
        .expect("could not write fixture");
}
