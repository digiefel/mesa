#import "kernel.typ" as _kernel

/// Read boundary polygons and width-aware paths from a named GDS cell.
///
/// Coordinates are returned in the user unit declared by the GDS library.
/// The corresponding physical scale in metres is available as
/// `layout.unit-meters`.
///
/// `path-tolerance` optionally simplifies the path centreline by a fraction of
/// that path's width before generating its two offset rails. For example, `2%`
/// limits the centreline deviation to two percent of the path width. Zero
/// preserves every path vertex.
#let gds(data, cell: none, layers: none, path-tolerance: 0) = {
  assert(type(data) == bytes, message: "gds data must be bytes")
  assert(type(cell) == str, message: "gds cell must be a string")
  assert(type(layers) == dictionary, message: "gds layers must be a dictionary")
  let path-tolerance = if type(path-tolerance) == ratio {
    path-tolerance / 100%
  } else {
    path-tolerance
  }
  assert(
    type(path-tolerance) in (int, float)
      and path-tolerance >= 0
      and path-tolerance <= 1,
    message: "gds path-tolerance must be between 0% and 100%",
  )
  for (name, layer) in layers {
    assert(
      type(layer) == array
        and layer.len() == 2
        and layer.all(value => type(value) == int),
      message: "gds layer " + name + " must be a (layer, datatype) pair",
    )
  }
  _kernel.gds-layout(data, cell, layers, path-tolerance)
}
