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
  label: none,
  ..style,
)
```

- `name` identifies the layer and its anchors.
- `thickness` is required and expressed in model units.
- `material` selects a style from the active material palette.
- `label` is Typst content associated with the layer.
- extra named arguments override the selected material style.

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

### `layer-stack`

```typ
layer-stack(
  body,
  size: (1, 1),
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
  length: 1cm,
  baseline: none,
  background: none,
  stroke: none,
  padding: none,
  debug: false,
)
```

`size` is optional and is `(x-width, y-depth)` in the same model units as layer
thickness. The default is a square sample of size `(1, 1)`.

The default camera is the front cross-section. `azimuth` rotates around the
vertical axis; `elevation` moves above the substrate plane. Changing either
angle reveals the depth of the same stack.

`light` uses the same angular coordinates as `camera`. `shading` chooses how
material colors respond to that light. `palette` changes material defaults;
per-layer style arguments still take precedence.

`rotate-labels` aligns labels with the projected layer face. Set it to `false`
to keep labels horizontal on the page.

`length`, `baseline`, `background`, `stroke`, `padding`, and `debug` are passed
to `cetz.canvas` with the same defaults and meaning. `length` specifies the
Typst length of one model unit.

The canvas `x`, `y`, and `z` arguments are not exposed. `layer-stack` controls
the coordinate basis to implement device coordinates and the camera.

### Coordinates

The package uses device coordinates:

- `x`: width;
- `y`: depth;
- `z`: height.

The substrate lies in the `x-y` plane. The `z` direction is normal to the
substrate plane, i.e. "up". The default camera shows the `x-z` cross-section.
