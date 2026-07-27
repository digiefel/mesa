#import "@preview/cetz:0.5.2"
#import "palette.typ": default-palette

#let _device-to-cetz = (
  (1, 0, 0, 0),
  (0, 0, 1, 0),
  (0, -1, 0, 0),
  (0, 0, 0, 1),
)

#let _merge-dictionaries(base, overrides) = {
  let merged = base
  for (key, value) in overrides {
    merged.insert(key, value)
  }
  merged
}

#let _direction(angles) = {
  let azimuth = angles.at("azimuth", default: 0deg)
  let elevation = angles.at("elevation", default: 0deg)
  let horizontal = calc.cos(elevation)

  (
    calc.sin(azimuth) * horizontal,
    calc.cos(azimuth) * horizontal,
    -calc.sin(elevation),
  )
}

#let _dot(a, b) = (
  a.at(0) * b.at(0)
  + a.at(1) * b.at(1)
  + a.at(2) * b.at(2)
)

#let _unit(vector) = {
  let length = calc.sqrt(_dot(vector, vector))
  if length == 0 {
    vector
  } else {
    vector.map(component => component / length)
  }
}

#let _add(a, b) = (
  a.at(0) + b.at(0),
  a.at(1) + b.at(1),
  a.at(2) + b.at(2),
)

#let _subtract(a, b) = (
  a.at(0) - b.at(0),
  a.at(1) - b.at(1),
  a.at(2) - b.at(2),
)

#let _scale(vector, factor) = (
  vector.at(0) * factor,
  vector.at(1) * factor,
  vector.at(2) * factor,
)

#let _toward-light(light) = _scale(_direction(light), -1)

#let _cross(a, b) = (
  a.at(1) * b.at(2) - a.at(2) * b.at(1),
  a.at(2) * b.at(0) - a.at(0) * b.at(2),
  a.at(0) * b.at(1) - a.at(1) * b.at(0),
)

#let _clip-polygon(points, distance, epsilon: 1e-6) = {
  if points.len() == 0 {
    return ()
  }

  let result = ()
  let previous = points.last()
  let previous-distance = distance(previous)
  let previous-inside = previous-distance > epsilon

  for current in points {
    let current-distance = distance(current)
    let current-inside = current-distance > epsilon
    if current-inside != previous-inside {
      let amount = (
        (epsilon - previous-distance)
        / (current-distance - previous-distance)
      )
      result.push(_add(
        previous,
        _scale(_subtract(current, previous), amount),
      ))
    }
    if current-inside {
      result.push(current)
    }
    previous = current
    previous-distance = current-distance
    previous-inside = current-inside
  }
  result
}

#let _signed-polygon-area(points) = {
  if points.len() < 3 {
    return 0
  }
  let twice-area = 0
  for index in range(points.len()) {
    let current = points.at(index)
    let next = points.at(calc.rem(index + 1, points.len()))
    let cross = current.at(0) * next.at(1) - current.at(1) * next.at(0)
    twice-area += cross
  }
  twice-area / 2
}

#let _polygon-area(points) = calc.abs(_signed-polygon-area(points))

#let _cross-2d(a, b) = a.at(0) * b.at(1) - a.at(1) * b.at(0)

#let _light-intensity(light) = {
  let value = light.at("intensity", default: 0.25)
  let value = if type(value) == ratio {
    value / 100%
  } else {
    value
  }
  assert(
    type(value) in (int, float) and value >= 0 and value <= 1,
    message: "light intensity must be between 0 and 1",
  )
  value
}

#let _resolve-bevel-value(value, thickness, name) = {
  assert(
    type(value) in (int, float, ratio),
    message: name + " bevel must be a number or ratio",
  )
  let result = if type(value) == ratio {
    thickness * (value / 100%)
  } else {
    value
  }
  assert(result >= 0, message: name + " bevel must be non-negative")
  result
}

#let _bevel-config(value, thickness, width, depth, fade-bottom) = {
  let value = if value == none {
    (top: 0, bottom: 0)
  } else if type(value) in (int, float, ratio) {
    (top: value, bottom: value)
  } else {
    assert(
      type(value) == dictionary,
      message: "bevel must be none, a number, a ratio, or a dictionary",
    )
    value
  }
  let top = _resolve-bevel-value(
    value.at("top", default: 0),
    thickness,
    "top",
  )
  let bottom = if fade-bottom == none {
    _resolve-bevel-value(
      value.at("bottom", default: 0),
      thickness,
      "bottom",
    )
  } else {
    0
  }
  assert(
    top + bottom < thickness,
    message: "top and bottom bevels must leave a positive vertical face",
  )
  assert(
    calc.max(top, bottom) < calc.min(width, depth) / 2,
    message: "bevel is too large for the layer footprint",
  )
  (top: top, bottom: bottom)
}

#let _face-brightness(normal, shading, light, visibility: 1) = {
  if shading == "none" {
    return 1
  }
  assert(
    shading in ("flat", "fancy"),
    message: "shading must be \"none\", \"flat\", or \"fancy\"",
  )

  let toward-light = _toward-light(light)
  let cosine = calc.max(0, _dot(normal, toward-light))
  let ambient = 1 - _light-intensity(light)
  let direct = _light-intensity(light) * visibility * cosine
  ambient + direct
}

#let _face-basis(face) = {
  let origin = face.points.first()
  let u = _unit(_subtract(face.points.at(1), origin))
  let v = _unit(_cross(face.normal, u))
  (
    origin: origin,
    u: u,
    v: v,
  )
}

#let _to-face-plane(point, basis) = {
  let relative = _subtract(point, basis.origin)
  (
    _dot(relative, basis.u),
    _dot(relative, basis.v),
    0,
  )
}

#let _shadow-polygons(receiver, receiver-index, faces, toward-light) = {
  let denominator = _dot(receiver.normal, toward-light)
  if denominator <= 1e-6 {
    return ()
  }

  let basis = _face-basis(receiver)
  let polygons = ()
  for (occluder-index, occluder) in faces.enumerate() {
    if occluder-index != receiver-index {
      let distance = point => _dot(
        _subtract(point, basis.origin),
        receiver.normal,
      )
      let clipped = _clip-polygon(occluder.points, distance)
      if clipped.len() >= 3 {
        let projected = clipped.map(point => {
          let amount = distance(point) / denominator
          _subtract(point, _scale(toward-light, amount))
        })
        let local = projected.map(point => _to-face-plane(point, basis))
        if _signed-polygon-area(local) < 0 {
          local = local.rev()
        }
        if _polygon-area(local) > 1e-8 {
          polygons.push(local)
        }
      }
    }
  }
  polygons
}

#let _point-in-convex(point, polygon, epsilon: 1e-6) = {
  for index in range(polygon.len()) {
    let start = polygon.at(index)
    let end = polygon.at(calc.rem(index + 1, polygon.len()))
    if _cross-2d(
      _subtract(end, start),
      _subtract(point, start),
    ) < -epsilon {
      return false
    }
  }
  true
}

#let _face-visibility(receiver, receiver-index, faces, shading, light) = {
  if shading == "none" or _light-intensity(light) == 0 {
    return 1
  }

  let toward-light = _toward-light(light)
  if _dot(receiver.normal, toward-light) <= 1e-6 {
    return 0
  }
  let polygons = _shadow-polygons(
    receiver,
    receiver-index,
    faces,
    toward-light,
  )
  if polygons.len() == 0 {
    return 1
  }

  let basis = _face-basis(receiver)
  let center = receiver.points.map(
    point => _to-face-plane(point, basis),
  ).fold(
    (0, 0, 0),
    (sum, point) => _add(sum, point),
  )
  center = _scale(center, 1 / receiver.points.len())
  if polygons.any(polygon => _point-in-convex(center, polygon)) {
    0
  } else {
    1
  }
}

#let _fade-config(value) = {
  let config = if type(value) == color {
    (start: 70%, end: 95%, color: value)
  } else {
    assert(
      type(value) == dictionary,
      message: "fade-bottom must be a color, dictionary, or none",
    )
    value
  }
  let start = config.at(
    "start",
    default: 100% - config.at("size", default: 30%),
  )
  let end = config.at("end", default: 95%)
  let target = config.at("color", default: white)
  assert(
    type(start) == ratio
      and type(end) == ratio
      and start >= 0%
      and start < end
      and end <= 100%,
    message: "fade-bottom requires 0% <= start < end <= 100%",
  )
  assert(
    type(target) == color,
    message: "fade-bottom color must be a color",
  )
  (start: start, end: end, color: target)
}

#let _mix-color(from, to, amount) = color.mix(
  (from, 100% - amount),
  (to, amount),
)

#let _fade-stops(from, to, start, end) = {
  let span = end - start
  (
    (from, 0%),
    (from, start),
    (_mix-color(from, to, 2.8%), start + span * 10%),
    (_mix-color(from, to, 10.4%), start + span * 20%),
    (_mix-color(from, to, 21.6%), start + span * 30%),
    (_mix-color(from, to, 35.2%), start + span * 40%),
    (_mix-color(from, to, 50%), start + span * 50%),
    (_mix-color(from, to, 64.8%), start + span * 60%),
    (_mix-color(from, to, 78.4%), start + span * 70%),
    (_mix-color(from, to, 89.6%), start + span * 80%),
    (_mix-color(from, to, 97.2%), start + span * 90%),
    (to, end),
    (to, 100%),
  )
}

#let _stroke-with-paint(value, paint) = {
  let base = stroke(value)

  stroke(
    paint: paint,
    thickness: base.thickness,
    cap: base.cap,
    join: base.join,
    dash: base.dash,
    miter-limit: base.miter-limit,
  )
}

#let _project(point, camera) = {
  let azimuth = camera.at("azimuth", default: 0deg)
  let elevation = camera.at("elevation", default: 0deg)
  let (x, y, z) = point

  (
    calc.cos(azimuth) * x + calc.sin(azimuth) * y,
    -calc.cos(elevation) * z
      + calc.sin(elevation)
        * (calc.sin(azimuth) * x - calc.cos(azimuth) * y),
  )
}

#let _face-horizontal(camera, face) = {
  let azimuth = camera.at("azimuth", default: 0deg)
  let elevation = camera.at("elevation", default: 0deg)
  if face in ("front", "back") {
    (
      calc.cos(azimuth),
      calc.sin(elevation) * calc.sin(azimuth),
    )
  } else if face == "right" {
    (
      calc.sin(azimuth),
      -calc.sin(elevation) * calc.cos(azimuth),
    )
  } else {
    (
      -calc.sin(azimuth),
      calc.sin(elevation) * calc.cos(azimuth),
    )
  }
}

#let project-face-content(body, camera, face) = {
  let elevation = camera.at("elevation", default: 0deg)
  let (horizontal-x, horizontal-y) = _face-horizontal(camera, face)
  let vertical-y = calc.cos(elevation)
  let shear = calc.atan2(horizontal-x, horizontal-y)

  std.skew(
    ay: shear,
    origin: center + horizon,
    std.scale(
      x: horizontal-x * 100%,
      y: vertical-y * 100%,
      origin: center + horizon,
      body,
    ),
  )
}

#let _face-content-angle(camera, face) = {
  let (horizontal-x, horizontal-y) = _face-horizontal(camera, face)
  calc.atan2(horizontal-x, horizontal-y)
}

#let _position-component-valid(value) = {
  type(value) in (
    int,
    float,
    length,
    ratio,
    type(50% + 1pt),
    type(center),
  )
}

#let _validate-position(value, name: "position") = {
  assert(
    type(value) == array
      and value.len() == 2
      and value.all(_position-component-valid),
    message: name + " must be an (x, z) pair",
  )
}

#let _automatic-label-z(style) = {
  let fade = style.at("fade-bottom", default: none)
  if fade == none {
    return 50%
  }

  let config = _fade-config(fade)
  let start = config.start / 100%
  let span = (config.end - config.start) / 100%
  let mass = start + span / 2
  let moment = start * start / 2 + span * (
    start / 2 + span * 3 / 20
  )
  (1 - moment / mass) * 100%
}

#let _resolve-position-component(value, extent, visual-middle, ctx, axis) = {
  let kind = type(value)
  if kind in (int, float) {
    float(value)
  } else if kind == ratio {
    extent * (value / 100%)
  } else if kind == length {
    float(value.to-absolute() / ctx.length)
  } else if kind == type(50% + 1pt) {
    (
      extent * (value.ratio / 100%)
        + float(value.length.to-absolute() / ctx.length)
    )
  } else {
    assert(
      value.axis() == if axis == "x" { "horizontal" } else { "vertical" },
      message: "invalid " + axis + " alignment in position",
    )
    let amount = if axis == "x" {
      if value in (left, start) {
        0
      } else if value == center {
        .5
      } else {
        1
      }
    } else {
      if value == bottom {
        0
      } else if value == horizon {
        visual-middle / 100%
      } else {
        1
      }
    }
    extent * amount
  }
}

#let _face-position(layer, face, position, ctx, camera) = {
  let horizontal-extent = if face in ("front", "back") {
    layer.width
  } else {
    layer.depth
  }
  let horizontal = _resolve-position-component(
    position.at(0),
    horizontal-extent,
    50%,
    ctx,
    "x",
  )
  let vertical = _resolve-position-component(
    position.at(1),
    layer.top - layer.bottom,
    layer.visual-middle,
    ctx,
    "z",
  )
  let point = if face == "front" {
    (horizontal, 0, layer.bottom + vertical)
  } else if face == "back" {
    (horizontal, layer.depth, layer.bottom + vertical)
  } else if face == "right" {
    (layer.width, layer.depth - horizontal, layer.bottom + vertical)
  } else {
    (0, horizontal, layer.bottom + vertical)
  }
  let projected = _project(point, camera)
  (projected.at(0), -projected.at(1), 0)
}

#let _dot-2d(a, b) = (
  a.at(0) * b.at(0)
  + a.at(1) * b.at(1)
)

#let _center-2d(points) = (
  points.map(point => point.at(0)).sum() / points.len(),
  points.map(point => point.at(1)).sum() / points.len(),
)

#let _fade-geometry(points, camera, start, end) = {
  let heights = points.map(point => point.at(2))
  let bottom = calc.min(..heights)
  let top = calc.max(..heights)
  let projected = points.map(point => _project(point, camera))
  let projected-top = (
    for (point, projected-point) in points.zip(projected) {
      if point.at(2) == top {
        (projected-point,)
      }
    }
  )
  let projected-bottom = (
    for (point, projected-point) in points.zip(projected) {
      if point.at(2) == bottom {
        (projected-point,)
      }
    }
  )
  let top-center = _center-2d(projected-top)
  let bottom-center = _center-2d(projected-bottom)
  let edge = (
    projected-top.at(1).at(0) - projected-top.at(0).at(0),
    projected-top.at(1).at(1) - projected-top.at(0).at(1),
  )
  if calc.abs(edge.at(0)) + calc.abs(edge.at(1)) < .000001 {
    return (
      angle: 90deg,
      start: start,
      end: end,
    )
  }
  let normal = (-edge.at(1), edge.at(0))
  let down = (
    bottom-center.at(0) - top-center.at(0),
    bottom-center.at(1) - top-center.at(1),
  )
  if _dot-2d(normal, down) < 0 {
    normal = (-normal.at(0), -normal.at(1))
  }

  let xs = projected.map(point => point.at(0))
  let ys = projected.map(point => point.at(1))
  let x-min = calc.min(..xs)
  let x-max = calc.max(..xs)
  let y-min = calc.min(..ys)
  let y-max = calc.max(..ys)
  let box-min = normal.at(0) * (
    if normal.at(0) >= 0 { x-min } else { x-max }
  ) + normal.at(1) * (
    if normal.at(1) >= 0 { y-min } else { y-max }
  )
  let box-max = normal.at(0) * (
    if normal.at(0) >= 0 { x-max } else { x-min }
  ) + normal.at(1) * (
    if normal.at(1) >= 0 { y-max } else { y-min }
  )
  let top-position = _dot-2d(top-center, normal)
  let bottom-position = _dot-2d(bottom-center, normal)
  let start-position = top-position + (
    bottom-position - top-position
  ) * (start / 100%)
  let end-position = top-position + (
    bottom-position - top-position
  ) * (end / 100%)
  let span = box-max - box-min

  (
    angle: calc.atan2(normal.at(0), normal.at(1)),
    start: calc.max(0, calc.min(1, (start-position - box-min) / span)) * 100%,
    end: calc.max(0, calc.min(1, (end-position - box-min) / span)) * 100%,
  )
}

#let _softened-stroke(value) = {
  let base = stroke(value)
  let paint = if base.paint == auto { black } else { base.paint }
  if type(paint) != color {
    return base
  }
  _stroke-with-paint(base, paint.transparentize(18%))
}

#let _draw-faded-outline(points, value, config, camera) = {
  let heights = points.map(point => point.at(2))
  let top = calc.max(..heights)
  let base = stroke(value)
  let paint = if base.paint == auto { black } else { base.paint }
  assert(
    type(paint) == color,
    message: "a face with fade-bottom must use a solid-color stroke",
  )

  for index in range(points.len()) {
    let start = points.at(index)
    let end = points.at(calc.rem(index + 1, points.len()))
    if start.at(2) == top and end.at(2) == top {
      cetz.draw.line(start, end, stroke: base)
    } else if start.at(2) != end.at(2) {
      let low = if start.at(2) < end.at(2) { start } else { end }
      let high = if start.at(2) < end.at(2) { end } else { start }
      let projected-high = _project(high, camera)
      let projected-low = _project(low, camera)
      let direction = (
        projected-low.at(0) - projected-high.at(0),
        projected-low.at(1) - projected-high.at(1),
      )
      let outline-paint = gradient.linear(
        .._fade-stops(
          paint,
          config.color,
          config.start,
          config.end,
        ),
        angle: calc.atan2(direction.at(0), direction.at(1)),
        relative: "self",
      )
      cetz.draw.line(
        high,
        low,
        stroke: _stroke-with-paint(base, outline-paint),
      )
    }
  }
}

#let _draw-faded-edge(high, low, value, config, camera) = {
  let base = stroke(value)
  let paint = if base.paint == auto { black } else { base.paint }
  let projected-high = _project(high, camera)
  let projected-low = _project(low, camera)
  let direction = (
    projected-low.at(0) - projected-high.at(0),
    projected-low.at(1) - projected-high.at(1),
  )
  let outline-paint = gradient.linear(
    .._fade-stops(
      paint,
      config.color,
      config.start,
      config.end,
    ),
    angle: calc.atan2(direction.at(0), direction.at(1)),
    relative: "self",
  )
  cetz.draw.line(
    high,
    low,
    stroke: _stroke-with-paint(base, outline-paint),
  )
}

#let _draw-beveled-outline(
  width,
  depth,
  bottom,
  top,
  top-bevel,
  bottom-bevel,
  style,
  camera,
) = {
  let value = style.at("stroke", default: 1pt + black)
  if value == none {
    return
  }
  let value = _softened-stroke(value)
  let top-shoulder = top - top-bevel
  let bottom-shoulder = bottom + bottom-bevel
  let outer = (
    (0, 0),
    (width, 0),
    (width, depth),
    (0, depth),
  )
  let top-ring = (
    (top-bevel, top-bevel, top),
    (width - top-bevel, top-bevel, top),
    (width - top-bevel, depth - top-bevel, top),
    (top-bevel, depth - top-bevel, top),
  )
  let bottom-ring = (
    (bottom-bevel, bottom-bevel, bottom),
    (width - bottom-bevel, bottom-bevel, bottom),
    (width - bottom-bevel, depth - bottom-bevel, bottom),
    (bottom-bevel, depth - bottom-bevel, bottom),
  )
  let fade-bottom = style.at("fade-bottom", default: none)
  let fade-config = if fade-bottom == none {
    none
  } else {
    _fade-config(fade-bottom)
  }

  cetz.draw.line(..top-ring, close: true, stroke: value)
  if fade-config == none {
    cetz.draw.line(..bottom-ring, close: true, stroke: value)
  }
  for index in range(4) {
    let corner = outer.at(index)
    let lower = (corner.at(0), corner.at(1), bottom-shoulder)
    let upper = (corner.at(0), corner.at(1), top-shoulder)
    if fade-config == none {
      let points = (
        bottom-ring.at(index),
        lower,
        upper,
        top-ring.at(index),
      )
      cetz.draw.line(..points, stroke: value)
    } else {
      _draw-faded-edge(upper, lower, value, fade-config, camera)
      let points = (upper, top-ring.at(index))
      cetz.draw.line(..points, stroke: value)
    }
  }
}

#let _queue-beveled-outline(
  width,
  depth,
  bottom,
  top,
  top-bevel,
  bottom-bevel,
  style,
  camera,
) = {
  cetz.draw.set-ctx(ctx => {
    ctx.shared-state.semi.outlines.push((
      width: width,
      depth: depth,
      bottom: bottom,
      top: top,
      top-bevel: top-bevel,
      bottom-bevel: bottom-bevel,
      style: style,
      camera: camera,
    ))
    ctx
  })
}

#let _material-style(material, variant, occurrence, local-style, palette) = {
  assert.eq(
    local-style.pos(),
    (),
    message: "layer accepts only named style overrides",
  )

  let family = if material == auto {
    palette.default
  } else {
    if material not in palette {
      panic("unknown material: " + repr(material))
    }
    palette.at(material)
  }

  let variants = if type(family) == array {
    family
  } else {
    (family,)
  }
  assert(variants.len() > 0, message: "material style family cannot be empty")

  let index = if variant == auto {
    calc.rem(occurrence, variants.len())
  } else {
    assert(
      type(variant) == int and variant >= 1 and variant <= variants.len(),
      message: "variant must be between 1 and "
        + str(variants.len())
        + " for material "
        + repr(material),
    )
    variant - 1
  }
  let style = variants.at(index)

  if type(style) == color {
    style = (fill: style)
  }
  assert(
    type(style) == dictionary,
    message: "material variants must be colors or style dictionaries",
  )

  let local-style = local-style.named()
  let result = _merge-dictionaries(style, local-style)
  if "fill" in local-style and "base-color" not in local-style {
    if type(local-style.fill) == color {
      result.base-color = local-style.fill
    } else if "base-color" in result {
      let _ = result.remove("base-color")
    }
  }
  result
}

#let _render-face(
  points,
  normal,
  style,
  shading,
  light,
  camera,
  visibility,
) = {
  let face-style = style
  let fade-bottom = face-style.at("fade-bottom", default: none)
  if "fade-bottom" in face-style {
    let _ = face-style.remove("fade-bottom")
  }
  if "base-color" in face-style {
    let _ = face-style.remove("base-color")
  }
  let fill = face-style.at("fill", default: none)
  let brightness = _face-brightness(
    normal,
    shading,
    light,
    visibility: visibility,
  )
  let outline = style.at("stroke", default: 1pt + black)
  let outline = if outline == none {
    none
  } else if shading == "fancy" {
    _softened-stroke(outline)
  } else {
    outline
  }
  let fades = fade-bottom != none and normal.at(2) == 0
  let config = if fades { _fade-config(fade-bottom) } else { none }
  let geometry = if fades {
    _fade-geometry(points, camera, config.start, config.end)
  } else {
    none
  }
  let shaded-fill = if type(fill) == color {
    fill.darken((1 - brightness) * 100%)
  } else {
    fill
  }

  if fade-bottom != none and normal.at(2) < 0 {
    return
  }

  if fades {
    assert(
      type(shaded-fill) == color,
      message: "a face with fade-bottom must use a solid-color fill",
    )
    face-style.fill = gradient.linear(
      .._fade-stops(
        shaded-fill,
        config.color,
        geometry.start,
        geometry.end,
      ),
      angle: geometry.angle,
      relative: "self",
    )
    face-style.stroke = none
  } else {
    face-style.fill = shaded-fill
    if "stroke" in face-style {
      face-style.stroke = outline
    }
  }

  cetz.draw.line(
    ..points,
    close: true,
    ..face-style,
  )

  if (
    shading in ("flat", "fancy")
    and fill != none
    and type(fill) != color
  ) {
    cetz.draw.line(
      ..points,
      close: true,
      fill: black.transparentize(brightness * 100%),
      stroke: none,
    )
  }

  if fades and outline != none {
    _draw-faded-outline(
      points,
      outline,
      config,
      camera,
    )
  }
}

#let _face(points, normal, style, shading, light, camera) = {
  if style.at("fade-bottom", default: none) != none and normal.at(2) < 0 {
    return
  }
  cetz.draw.set-ctx(ctx => {
    let state = ctx.shared-state.semi
    state.faces.push((
      layer: state.layers.len(),
      points: points,
      normal: normal,
      style: style,
      shading: shading,
      light: light,
      camera: camera,
    ))
    ctx.shared-state.semi = state
    ctx
  })
}

#let _draw-scene() = {
  cetz.draw.set-ctx(ctx => {
    let state = ctx.shared-state.semi
    let diagnostics = ()
    let layer-names = state.layers.keys()
    for (index, face) in state.faces.enumerate() {
      let visibility = _face-visibility(
        face,
        index,
        state.faces,
        face.shading,
        face.light,
      )
      let cosine = calc.max(0, _dot(
        face.normal,
        _toward-light(face.light),
      ))
      diagnostics.push((
        index: index,
        layer: face.layer,
        layer-name: if face.layer < layer-names.len() {
          layer-names.at(face.layer)
        } else {
          str(face.layer)
        },
        points: face.points,
        center: _scale(
          face.points.fold(
            (0, 0, 0),
            (sum, point) => _add(sum, point),
          ),
          1 / face.points.len(),
        ),
        normal: face.normal,
        cosine: cosine,
        visibility: visibility,
        brightness: _face-brightness(
          face.normal,
          face.shading,
          face.light,
          visibility: visibility,
        ),
      ))
    }
    state.face-diagnostics = diagnostics
    ctx.shared-state.semi = state
    ctx
  })

  cetz.draw.get-ctx(ctx => {
    let state = ctx.shared-state.semi
    cetz.draw.on-layer(-1, {
      for (index, face) in state.faces.enumerate() {
        _render-face(
          face.points,
          face.normal,
          face.style,
          face.shading,
          face.light,
          face.camera,
          state.face-diagnostics.at(index).visibility,
        )
      }
      for outline in state.outlines {
        _draw-beveled-outline(
          outline.width,
          outline.depth,
          outline.bottom,
          outline.top,
          outline.top-bevel,
          outline.bottom-bevel,
          outline.style,
          outline.camera,
        )
      }
    })
  })
}

#let layer(
  name,
  thickness: none,
  material: auto,
  variant: auto,
  label: none,
  label-transform: auto,
  label-position: (center, horizon),
  bevel: auto,
  ..style,
) = {
  assert(type(name) == str, message: "layer name must be a string")
  assert(
    material == auto or type(material) == str,
    message: "material must be a string or auto",
  )
  assert(
    variant == auto or type(variant) == int,
    message: "variant must be an integer or auto",
  )
  assert(
    label-transform == auto
      or label-transform in ("none", "rotate", "project"),
    message: "label-transform must be auto, \"none\", \"rotate\", or \"project\"",
  )
  _validate-position(label-position, name: "label-position")
  assert(
    type(thickness) in (int, float),
    message: "layer thickness must be a number",
  )
  assert(thickness > 0, message: "layer thickness must be positive")

  cetz.draw.get-ctx(ctx => {
    let state = ctx.shared-state.at("semi", default: none)
    assert(
      state != none,
      message: "layer must be used inside layer-stack",
    )

    let (width, depth) = state.size
    let bottom = state.height
    let top = bottom + thickness
    let middle = (bottom + top) / 2
    let family-name = if material == auto { "default" } else { material }
    let occurrence = state.material-counts.at(family-name, default: 0)
    let resolved-style = _material-style(
      material,
      variant,
      occurrence,
      style,
      state.palette,
    )
    let visual-middle = _automatic-label-z(resolved-style)
    let bevel = if state.shading == "fancy" {
      _bevel-config(
        if bevel == auto { state.bevel } else { bevel },
        thickness,
        width,
        depth,
        resolved-style.at("fade-bottom", default: none),
      )
    } else {
      (top: 0, bottom: 0)
    }
    let top-bevel = bevel.top
    let bottom-bevel = bevel.bottom
    let top-shoulder = top - top-bevel
    let bottom-shoulder = bottom + bottom-bevel
    let render-style = resolved-style
    if state.shading == "fancy" and not state.internal-strokes {
      render-style.stroke = none
    }
    let bevel-style = render-style
    let bevel-color = resolved-style.at("base-color", default: none)
    if bevel-color != none {
      bevel-style.fill = bevel-color
    }

    cetz.draw.group(
      name: name,
      {
        _face(
          (
            (0, depth, bottom-shoulder),
            (width, depth, bottom-shoulder),
            (width, depth, top-shoulder),
            (0, depth, top-shoulder),
          ),
          (0, 1, 0),
          render-style,
          state.shading,
          state.light,
          state.camera,
        )
        _face(
          (
            (bottom-bevel, bottom-bevel, bottom),
            (bottom-bevel, depth - bottom-bevel, bottom),
            (width - bottom-bevel, depth - bottom-bevel, bottom),
            (width - bottom-bevel, bottom-bevel, bottom),
          ),
          (0, 0, -1),
          render-style,
          state.shading,
          state.light,
          state.camera,
        )
        _face(
          (
            (0, 0, bottom-shoulder),
            (0, 0, top-shoulder),
            (0, depth, top-shoulder),
            (0, depth, bottom-shoulder),
          ),
          (-1, 0, 0),
          render-style,
          state.shading,
          state.light,
          state.camera,
        )
        _face(
          (
            (width, 0, bottom-shoulder),
            (width, depth, bottom-shoulder),
            (width, depth, top-shoulder),
            (width, 0, top-shoulder),
          ),
          (1, 0, 0),
          render-style,
          state.shading,
          state.light,
          state.camera,
        )
        if top-bevel > 0 {
          _face(
            (
              (0, depth, top-shoulder),
              (width, depth, top-shoulder),
              (width - top-bevel, depth - top-bevel, top),
              (top-bevel, depth - top-bevel, top),
            ),
            _unit((0, 1, 1)),
            bevel-style,
            state.shading,
            state.light,
            state.camera,
          )
          _face(
            (
              (0, 0, top-shoulder),
              (top-bevel, top-bevel, top),
              (width - top-bevel, top-bevel, top),
              (width, 0, top-shoulder),
            ),
            _unit((0, -1, 1)),
            bevel-style,
            state.shading,
            state.light,
            state.camera,
          )
          _face(
            (
              (0, 0, top-shoulder),
              (0, depth, top-shoulder),
              (top-bevel, depth - top-bevel, top),
              (top-bevel, top-bevel, top),
            ),
            _unit((-1, 0, 1)),
            bevel-style,
            state.shading,
            state.light,
            state.camera,
          )
          _face(
            (
              (width, 0, top-shoulder),
              (width - top-bevel, top-bevel, top),
              (width - top-bevel, depth - top-bevel, top),
              (width, depth, top-shoulder),
            ),
            _unit((1, 0, 1)),
            bevel-style,
            state.shading,
            state.light,
            state.camera,
          )
        }
        if bottom-bevel > 0 {
          _face(
            (
              (0, depth, bottom-shoulder),
              (bottom-bevel, depth - bottom-bevel, bottom),
              (width - bottom-bevel, depth - bottom-bevel, bottom),
              (width, depth, bottom-shoulder),
            ),
            _unit((0, 1, -1)),
            bevel-style,
            state.shading,
            state.light,
            state.camera,
          )
          _face(
            (
              (0, 0, bottom-shoulder),
              (width, 0, bottom-shoulder),
              (width - bottom-bevel, bottom-bevel, bottom),
              (bottom-bevel, bottom-bevel, bottom),
            ),
            _unit((0, -1, -1)),
            bevel-style,
            state.shading,
            state.light,
            state.camera,
          )
          _face(
            (
              (0, 0, bottom-shoulder),
              (bottom-bevel, bottom-bevel, bottom),
              (bottom-bevel, depth - bottom-bevel, bottom),
              (0, depth, bottom-shoulder),
            ),
            _unit((-1, 0, -1)),
            bevel-style,
            state.shading,
            state.light,
            state.camera,
          )
          _face(
            (
              (width, 0, bottom-shoulder),
              (width, depth, bottom-shoulder),
              (width - bottom-bevel, depth - bottom-bevel, bottom),
              (width - bottom-bevel, bottom-bevel, bottom),
            ),
            _unit((1, 0, -1)),
            bevel-style,
            state.shading,
            state.light,
            state.camera,
          )
        }
        _face(
          (
            (top-bevel, top-bevel, top),
            (width - top-bevel, top-bevel, top),
            (width - top-bevel, depth - top-bevel, top),
            (top-bevel, depth - top-bevel, top),
          ),
          (0, 0, 1),
          render-style,
          state.shading,
          state.light,
          state.camera,
        )
        _face(
          (
            (0, 0, bottom-shoulder),
            (width, 0, bottom-shoulder),
            (width, 0, top-shoulder),
            (0, 0, top-shoulder),
          ),
          (0, -1, 0),
          render-style,
          state.shading,
          state.light,
          state.camera,
        )
        if state.shading == "fancy" and not state.internal-strokes {
          _queue-beveled-outline(
            width,
            depth,
            bottom,
            top,
            top-bevel,
            bottom-bevel,
            resolved-style,
            state.camera,
          )
        }

        cetz.draw.anchor("bottom", (width / 2, depth / 2, bottom))
        cetz.draw.anchor("top", (width / 2, depth / 2, top))
        cetz.draw.anchor("center", (width / 2, depth / 2, middle))
        cetz.draw.anchor("front", (width / 2, 0, middle))
        cetz.draw.anchor("back", (width / 2, depth, middle))
        cetz.draw.anchor("left", (0, depth / 2, middle))
        cetz.draw.anchor("right", (width, depth / 2, middle))
        cetz.draw.anchor("front-left", (0, 0, middle))
        cetz.draw.anchor("front-right", (width, 0, middle))
        cetz.draw.anchor("back-left", (0, depth, middle))
        cetz.draw.anchor("back-right", (width, depth, middle))
        cetz.draw.anchor("front-left-bottom", (0, 0, bottom))
        cetz.draw.anchor("front-left-top", (0, 0, top))
        cetz.draw.anchor("front-right-bottom", (width, 0, bottom))
        cetz.draw.anchor("front-right-top", (width, 0, top))
        cetz.draw.anchor("back-left-bottom", (0, depth, bottom))
        cetz.draw.anchor("back-left-top", (0, depth, top))
        cetz.draw.anchor("back-right-bottom", (width, depth, bottom))
        cetz.draw.anchor("back-right-top", (width, depth, top))
      },
    )

    cetz.draw.set-ctx(ctx => {
      ctx.shared-state.semi.height = top
      ctx.shared-state.semi.material-counts.insert(
        family-name,
        occurrence + 1,
      )
      ctx.shared-state.semi.layers.insert(name, (
        width: width,
        depth: depth,
        bottom: bottom,
        top: top,
        visual-middle: visual-middle,
      ))
      if label != none {
        ctx.shared-state.semi.face-contents.push((
          target: name,
          body: label,
          face: auto,
          transform: label-transform,
          anchor: "center",
          position: label-position,
        ))
      }
      ctx
    })
  })
}

#let face-content(
  target,
  body,
  face: auto,
  transform: "project",
  anchor: "center",
  position: (center, horizon),
) = {
  assert(type(target) == str, message: "face-content target must be a string")
  assert(
    face == auto or face in ("front", "back", "left", "right"),
    message: "face must be auto, \"front\", \"back\", \"left\", or \"right\"",
  )
  assert(
    transform in ("none", "rotate", "project"),
    message: "transform must be \"none\", \"rotate\", or \"project\"",
  )
  assert(type(anchor) == str, message: "anchor must be a string")
  _validate-position(position)

  cetz.draw.set-ctx(ctx => {
    let state = ctx.shared-state.at("semi", default: none)
    assert(
      state != none,
      message: "face-content must be used inside layer-stack",
    )
    ctx.shared-state.semi.face-contents.push((
      target: target,
      body: body,
      face: face,
      transform: transform,
      anchor: anchor,
      position: position,
    ))
    ctx
  })
}

#let layer-stack(
  body,
  size: (80, 50),
  camera: (
    azimuth: 0deg,
    elevation: 0deg,
  ),
  shading: "flat",
  light: (
    azimuth: -45deg,
    elevation: 60deg,
    intensity: 0.25,
  ),
  bevel: (top: 0.5, bottom: 0.25),
  internal-strokes: false,
  palette: (:),
  label-transform: "project",
  length: .8mm,
  baseline: none,
  background: none,
  stroke: none,
  padding: none,
  debug: none,
  canvas-debug: false,
) = {
  assert(
    type(size) == array and size.len() == 2,
    message: "size must be (width, depth)",
  )
  assert(size.all(value => value > 0), message: "size values must be positive")
  assert(type(camera) == dictionary, message: "camera must be a dictionary")
  assert(type(light) == dictionary, message: "light must be a dictionary")
  assert(
    type(internal-strokes) == bool,
    message: "internal-strokes must be a boolean",
  )
  assert(type(palette) == dictionary, message: "palette must be a dictionary")
  assert(
    type(canvas-debug) == bool,
    message: "canvas-debug must be a boolean",
  )
  assert(
    label-transform in ("none", "rotate", "project"),
    message: "label-transform must be \"none\", \"rotate\", or \"project\"",
  )
  assert(
    shading in ("none", "flat", "fancy"),
    message: "unknown shading mode",
  )

  let active-palette = _merge-dictionaries(default-palette, palette)
  let azimuth = camera.at("azimuth", default: 0deg)
  let elevation = camera.at("elevation", default: 0deg)

  cetz.canvas(
    length: length,
    baseline: baseline,
    background: background,
    stroke: stroke,
    padding: padding,
    debug: canvas-debug,
    {
      cetz.draw.set-ctx(ctx => {
        ctx.shared-state.semi = (
          size: size,
          height: 0,
          faces: (),
          face-diagnostics: (),
          outlines: (),
          face-contents: (),
          layers: (:),
          material-counts: (:),
          palette: active-palette,
          shading: shading,
          light: light,
          bevel: bevel,
          internal-strokes: internal-strokes,
          camera: camera,
        )
        ctx
      })

      cetz.draw.ortho(
        x: elevation,
        y: azimuth,
        sorted: true,
        cull-face: none,
        {
          cetz.draw.transform(_device-to-cetz)
          body
          _draw-scene()
          if debug != none {
            cetz.draw.on-layer(2, debug)
          }
        },
      )

      cetz.draw.on-layer(1, {
        cetz.draw.get-ctx(ctx => {
          let label-face = if calc.sin(azimuth) < 0 {
            "back"
          } else {
            "front"
          }
          for item in ctx.shared-state.semi.face-contents {
            let face = if item.face == auto {
              label-face
            } else {
              item.face
            }
            let layer = ctx.shared-state.semi.layers.at(
              item.target,
              default: none,
            )
            assert(
              layer != none,
              message: "unknown face-content target: " + repr(item.target),
            )
            let position = _face-position(
              layer,
              face,
              item.position,
              ctx,
              camera,
            )
            let transform = if item.transform == auto {
              label-transform
            } else {
              item.transform
            }
            let body = if transform == "project" {
              project-face-content(item.body, camera, face)
            } else {
              item.body
            }
            cetz.draw.content(
              position,
              body,
              anchor: item.anchor,
              angle: if transform == "rotate" {
                _face-content-angle(camera, face)
              } else {
                0deg
              },
            )
          }
        })
      })
    },
  )
}
