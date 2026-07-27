#let geometry-kernel = plugin("plugin/semi_geometry.wasm")

#let protocol-version = 1
#let grid-scale = 1000

#let version() = str(geometry-kernel.kernel_version())

#let _encode-point(point) = point.map(
  component => int(calc.round(component * grid-scale)),
)

#let _decode-point(point) = point.map(
  component => component / grid-scale,
)

#let _encode-shapes(shapes) = shapes.map(
  shape => shape.map(
    contour => contour.map(_encode-point),
  ),
)

#let _decode-shapes(shapes) = shapes.map(
  shape => shape.map(
    contour => contour.map(_decode-point),
  ),
)

#let _encode-volume(volume, material) = (
  shapes: _encode-shapes(volume.shapes),
  bottom: int(calc.round(volume.bottom * grid-scale)),
  top: int(calc.round(volume.top * grid-scale)),
  material: material,
)

#let difference(subject, mask) = {
  let result = cbor(geometry-kernel.difference(cbor.encode((
    version: protocol-version,
    subject: _encode-shapes(subject),
    mask: _encode-shapes(mask),
  ))))
  assert.eq(result.version, protocol-version)
  _decode-shapes(result.shapes)
}

#let cross-section(shapes, y) = {
  let result = cbor(geometry-kernel.cross_section(cbor.encode((
    version: protocol-version,
    shapes: _encode-shapes(shapes),
    y: int(calc.round(y * grid-scale)),
  ))))
  assert.eq(result.version, protocol-version)
  result.intervals.map(interval => interval.map(
    component => component / grid-scale,
  ))
}

#let clip-y(shapes, y, keep: "positive") = {
  assert(
    keep in ("positive", "negative"),
    message: "clip-y keep must be \"positive\" or \"negative\"",
  )
  let result = cbor(geometry-kernel.clip_y(cbor.encode((
    version: protocol-version,
    shapes: _encode-shapes(shapes),
    y: int(calc.round(y * grid-scale)),
    positive: keep == "positive",
  ))))
  assert.eq(result.version, protocol-version)
  _decode-shapes(result.shapes)
}

#let scene-topology(volumes) = {
  let result = cbor(geometry-kernel.scene_topology(cbor.encode((
    version: protocol-version,
    volumes: volumes.enumerate().map(
      ((index, volume)) => _encode-volume(volume, index),
    ),
  ))))
  assert.eq(result.version, protocol-version)
  result.edges.map(edge => (
    start: _decode-point(edge.start),
    end: _decode-point(edge.end),
    kind: edge.kind,
    faces: edge.faces,
  ))
}
