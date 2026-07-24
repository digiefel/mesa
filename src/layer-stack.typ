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

#let _faded-stroke(value, fade-bottom) = {
  let base = stroke(value)
  let paint = if base.paint == auto { black } else { base.paint }
  assert(
    type(paint) == color,
    message: "a face with fade-bottom must use a solid-color stroke",
  )

  stroke(
    paint: gradient.linear(
      (paint, 35%),
      (fade-bottom, 100%),
      angle: 90deg,
      relative: "self",
    ),
    thickness: base.thickness,
    cap: base.cap,
    join: base.join,
    dash: base.dash,
    miter-limit: base.miter-limit,
  )
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

#let _face(points, normal, style, shading, light) = {
  let face-style = style
  let fade-bottom = face-style.at("fade-bottom", default: none)
  if "fade-bottom" in face-style {
    face-style.remove("fade-bottom")
  }
  let fill = face-style.at("fill", default: none)
  let brightness = _face-brightness(normal, shading, light)

  if type(fill) == color {
    face-style.fill = fill.darken((1 - brightness) * 38%)
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

  if fade-bottom != none and normal.at(2) == 0 {
    assert(
      type(fade-bottom) == color,
      message: "fade-bottom must be a color or none",
    )
    cetz.draw.line(
      ..points,
      close: true,
      fill: gradient.linear(
        (fade-bottom.transparentize(100%), 35%),
        (fade-bottom, 100%),
        angle: 90deg,
        relative: "self",
      ),
      stroke: _faded-stroke(
        face-style.at("stroke", default: 1pt + black),
        fade-bottom,
      ),
    )
  }
}

#let layer(
  name,
  thickness: none,
  material: auto,
  variant: auto,
  label: none,
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
      },
    )

    cetz.draw.set-ctx(ctx => {
      ctx.shared-state.semi.height = top
      ctx.shared-state.semi.material-counts.insert(
        family-name,
        occurrence + 1,
      )
      if label != none {
        ctx.shared-state.semi.labels.push((
          name: name,
          body: label,
        ))
      }
      ctx
    })
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
  rotate-labels: true,
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
  assert(type(rotate-labels) == bool, message: "rotate-labels must be a boolean")
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
          labels: (),
          material-counts: (:),
          palette: active-palette,
          shading: shading,
          light: light,
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
          for label in ctx.shared-state.semi.labels {
            let position = label.name + "." + label-face
            cetz.draw.content(
              position,
              label.body,
              anchor: "center",
              angle: if rotate-labels {
                label.name + "." + label-face + "-right"
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
