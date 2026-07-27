#import "kernel.typ" as _kernel

/// Read boundary polygons and width-aware paths from a named GDS cell.
///
/// `path-tolerance` optionally simplifies the path centreline in nanometres
/// before generating its two offset rails. Zero preserves every path vertex.
#let gds(data, cell: none, layers: none, path-tolerance: 0) = {
  assert(type(data) == bytes, message: "gds data must be bytes")
  assert(type(cell) == str, message: "gds cell must be a string")
  assert(type(layers) == dictionary, message: "gds layers must be a dictionary")
  assert(
    type(path-tolerance) in (int, float) and path-tolerance >= 0,
    message: "gds path-tolerance must be a non-negative number",
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
