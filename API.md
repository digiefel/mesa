# Layer-stack API

How to describe a layer stack and render it in 2D or 3D.

## Scope

Supports:

- horizontal layers with a shared rectangular footprint;
- a thickness, material, and optional label for each layer;
- 2D cross-section and 3D oblique rendering from the same stack;
- package defaults with per-layer style overrides;
- different styles of shading and lighting effects.

## Proposed API

```typ
#import "src/lib.typ" as semi

#let sample = {
  import semi: *

  layer(
    "substrate",
    thickness: 1.2,
    material: "substrate",
    label: [substrate],
  )
  layer(
    "dielectric",
    thickness: 0.18,
    material: "dielectric",
    label: [dielectric],
  )
  layer(
    "metal",
    thickness: 0.25,
    material: "metal",
    label: [metal],
  )
  layer(
    "resist",
    thickness: 0.7,
    material: "resist",
    label: [resist],
  )

  // any kind of CeTZ annotation here
}

#semi.layer-stack(sample)

#semi.layer-stack(
  sample,
  camera: (
    azimuth: 35deg,
    elevation: 25deg,
  ),
  shading: "flat",
  light: (
    azimuth: -45deg,
    elevation: 60deg,
  ),
)
```

`layer-stack` owns the CeTZ canvas. The functions in its body add layers to the
stack in order. Camera, shading, and lighting can be customized with the
respective `layer-stack` arguments. Other CeTZ components and primitives can be
used in the body to annotate the stack. Relevant CeTZ canvas arguments are also
available through `layer-stack`.

## Model

### `layer`

```typ
layer(
  name,
  thickness: none,
  material: auto,
  variant: auto,
  label: none,
  label-transform: auto,
  ..style,
)
```

- `name` identifies the layer and its anchors.
- `thickness` is required and expressed in model units.
- `material` selects a style from the active material palette.
- `variant` selects a 1-based style variant. By default, variants advance and
  cycle independently for each material.
- `label` is Typst content associated with the layer.
- `label-transform` overrides the stack's label transformation for this layer.
  Its default, `auto`, inherits the `layer-stack` setting.
- extra named arguments override the selected material style.

```typ
layer("metal-1", thickness: 0.3, material: "metal")
layer("metal-2", thickness: 0.3, material: "metal")
layer("metal-3", thickness: 0.3, material: "metal", variant: 1)
```

The first two layers use successive metal variants. The third explicitly uses
variant 1. It still advances the metal occurrence counter.

Material fills can be colors or the package's `hatch`, `crosshatch`, and `dots`
tilings:

```typ
layer(
  "metal",
  thickness: 0.4,
  fill: hatch(
    background: rgb("#d8c27a"),
    color: rgb("#8e762c"),
  ),
)
```

`fade-bottom` fades a material between two depths measured from its top:

```typ
fade-bottom: (start: 70%, end: 95%, color: white)
```

A palette entry is either one style or an array of variants:

```typ
#semi.layer-stack(
  sample,
  palette: (
    metal: (
      (fill: rgb("#d9b44a")),
      (fill: hatch(
        background: rgb("#d7a17c"),
        color: rgb("#985f3d"),
      )),
    ),
  ),
)
```

Automatic selection starts with the first variant, advances for every layer in
that material family, and wraps at the end of the array.

### `layer-stack`

```typ
layer-stack(
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
)
```

Model coordinates represent nanometres by default. `size` is `(x-width,
y-depth)` in the same units as layer thickness; its default is `(80, 50)`.

The default camera is the front cross-section. `azimuth` rotates around the
vertical axis; `elevation` moves above the substrate plane. Changing either
angle reveals the depth of the same stack.

`light` uses the same angular coordinates as `camera`. `shading` chooses how
material colors respond to that light. `palette` changes material defaults;
per-layer style arguments still take precedence.

`label-transform` controls how labels follow the layer face. `"project"`
applies the face's full orthographic projection, including foreshortening and
shear. `"rotate"` only aligns the baseline with the face, while `"none"` keeps
labels horizontal on the page.

`length`, `baseline`, `background`, `stroke`, `padding`, and `debug` are passed
to `cetz.canvas`. `length` specifies the rendered length of one model unit and
defaults to `.8mm`. The default front view is 64 mm wide; an oblique view is
approximately half-column width on an A4 page.

The canvas `x`, `y`, and `z` arguments are not exposed. `layer-stack` controls
the coordinate basis to implement device coordinates and the camera.

### Coordinates

The package uses device coordinates:

- `x`: width;
- `y`: depth;
- `z`: height.

The substrate lies in the `x-y` plane. The `z` direction is normal to the
substrate plane, i.e. "up". The default camera shows the `x-z` cross-section.
