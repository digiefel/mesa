#import "@preview/cetz:0.5.2"
#import "palette.typ": default-palette

#let _device-to-cetz = (
  (1, 0, 0, 0),
  (0, 0, 1, 0),
  (0, 1, 0, 0),
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
    -calc.cos(azimuth) * horizontal,
    calc.sin(elevation),
  )
}

#let _dot(a, b) = (
  a.at(0) * b.at(0)
  + a.at(1) * b.at(1)
  + a.at(2) * b.at(2)
)

#let _face-brightness(normal, shading, light) = {
  if shading == "none" {
    return 1
  }
  assert.eq(
    shading,
    "flat",
    message: "shading must be \"none\" or \"flat\"",
  )

  let diffuse = calc.max(0, _dot(normal, _direction(light)))
  0.58 + 0.42 * diffuse
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
    calc.cos(azimuth) * x - calc.sin(azimuth) * y,
    -calc.cos(elevation) * z
      + calc.sin(elevation)
        * (calc.sin(azimuth) * x + calc.cos(azimuth) * y),
  )
}

#let _project-face-content(body, camera, face) = {
  let azimuth = camera.at("azimuth", default: 0deg)
  let elevation = camera.at("elevation", default: 0deg)
  let (horizontal-x, horizontal-y) = if face in ("front", "back") {
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

#let _face-direction-anchor(name, face) = {
  if face == "front" {
    name + ".front-right"
  } else if face == "back" {
    name + ".back-right"
  } else if face == "right" {
    name + ".front-right"
  } else {
    name + ".back-left"
  }
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

  _merge-dictionaries(style, local-style.named())
}

#let _face(points, normal, style, shading, light, camera) = {
  let face-style = style
  let fade-bottom = face-style.at("fade-bottom", default: none)
  if "fade-bottom" in face-style {
    let _ = face-style.remove("fade-bottom")
  }
  let fill = face-style.at("fill", default: none)
  let brightness = _face-brightness(normal, shading, light)
  let fades = fade-bottom != none and normal.at(2) == 0
  let config = if fades { _fade-config(fade-bottom) } else { none }
  let geometry = if fades {
    _fade-geometry(points, camera, config.start, config.end)
  } else {
    none
  }
  let shaded-fill = if type(fill) == color {
    fill.darken((1 - brightness) * 38%)
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
  }

  cetz.draw.line(
    ..points,
    close: true,
    ..face-style,
  )

  if shading == "flat" and fill != none and type(fill) != color {
    cetz.draw.line(
      ..points,
      close: true,
      fill: black.transparentize(100% - (1 - brightness) * 24%),
      stroke: none,
    )
  }

  if fades {
    _draw-faded-outline(
      points,
      style.at("stroke", default: 1pt + black),
      config,
      camera,
    )
  }
}

#let layer(
  name,
  thickness: none,
  material: auto,
  variant: auto,
  label: none,
  label-transform: auto,
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

    cetz.draw.group(
      name: name,
      {
        _face(
          (
            (0, depth, bottom),
            (width, depth, bottom),
            (width, depth, top),
            (0, depth, top),
          ),
          (0, 1, 0),
          resolved-style,
          state.shading,
          state.light,
          state.camera,
        )
        _face(
          (
            (0, 0, bottom),
            (0, depth, bottom),
            (width, depth, bottom),
            (width, 0, bottom),
          ),
          (0, 0, -1),
          resolved-style,
          state.shading,
          state.light,
          state.camera,
        )
        _face(
          (
            (0, 0, bottom),
            (0, 0, top),
            (0, depth, top),
            (0, depth, bottom),
          ),
          (-1, 0, 0),
          resolved-style,
          state.shading,
          state.light,
          state.camera,
        )
        _face(
          (
            (width, 0, bottom),
            (width, depth, bottom),
            (width, depth, top),
            (width, 0, top),
          ),
          (1, 0, 0),
          resolved-style,
          state.shading,
          state.light,
          state.camera,
        )
        _face(
          (
            (0, 0, top),
            (width, 0, top),
            (width, depth, top),
            (0, depth, top),
          ),
          (0, 0, 1),
          resolved-style,
          state.shading,
          state.light,
          state.camera,
        )
        _face(
          (
            (0, 0, bottom),
            (width, 0, bottom),
            (width, 0, top),
            (0, 0, top),
          ),
          (0, -1, 0),
          resolved-style,
          state.shading,
          state.light,
          state.camera,
        )

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
      if label != none {
        ctx.shared-state.semi.face-contents.push((
          target: name,
          body: label,
          face: auto,
          transform: label-transform,
          anchor: "center",
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
  ),
  palette: (:),
  label-transform: "project",
  length: .8mm,
  baseline: none,
  background: none,
  stroke: none,
  padding: none,
  debug: false,
) = {
  assert(
    type(size) == array and size.len() == 2,
    message: "size must be (width, depth)",
  )
  assert(size.all(value => value > 0), message: "size values must be positive")
  assert(type(camera) == dictionary, message: "camera must be a dictionary")
  assert(type(light) == dictionary, message: "light must be a dictionary")
  assert(type(palette) == dictionary, message: "palette must be a dictionary")
  assert(
    label-transform in ("none", "rotate", "project"),
    message: "label-transform must be \"none\", \"rotate\", or \"project\"",
  )
  assert(shading in ("none", "flat"), message: "unknown shading mode")

  let active-palette = _merge-dictionaries(default-palette, palette)
  let azimuth = camera.at("azimuth", default: 0deg)
  let elevation = camera.at("elevation", default: 0deg)

  cetz.canvas(
    length: length,
    baseline: baseline,
    background: background,
    stroke: stroke,
    padding: padding,
    debug: debug,
    {
      cetz.draw.set-ctx(ctx => {
        ctx.shared-state.semi = (
          size: size,
          height: 0,
          face-contents: (),
          material-counts: (:),
          palette: active-palette,
          shading: shading,
          light: light,
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
        },
      )

      cetz.draw.on-layer(1, {
        cetz.draw.get-ctx(ctx => {
          let label-face = if calc.sin(azimuth) > 0 {
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
            let position = item.target + "." + face
            let transform = if item.transform == auto {
              label-transform
            } else {
              item.transform
            }
            let body = if transform == "project" {
              _project-face-content(item.body, camera, face)
            } else {
              item.body
            }
            cetz.draw.content(
              position,
              body,
              anchor: item.anchor,
              angle: if transform == "rotate" {
                _face-direction-anchor(item.target, face)
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
