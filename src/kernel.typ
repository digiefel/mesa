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

#let difference(subject, mask) = {
  let result = cbor(geometry-kernel.difference(cbor.encode((
    version: protocol-version,
    subject: _encode-shapes(subject),
    mask: _encode-shapes(mask),
  ))))
  assert.eq(result.version, protocol-version)
  _decode-shapes(result.shapes)
}
