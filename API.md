# Layer-stack API

How to describe a layer stack and render it in 2D or 3D.

## Scope

Supports:

- horizontal layers with a shared rectangular footprint;
- a thickness, material, and optional label for each layer;
- 2D cross-section and 3D oblique rendering from the same stack;
- package defaults with per-layer style overrides.

Patterned layers, fabrication operations, layout import, and general-purpose
annotations are outside this slice.

## Proposed API

```typ
#import "@preview/cetz:0.5.2"
#import "src/lib.typ": layer, layer-stack

#let sample = (
  layer(
    id: "substrate",
    thickness: 1.2,
    material: "silicon",
    label: [Si],
  ),
  layer(
    id: "oxide",
    thickness: 0.18,
    material: "oxide",
    label: [SiO#sub[2]],
  ),
  layer(
    id: "resist",
    thickness: 0.7,
    material: "resist",
    label: [resist],
  ),
)

#cetz.canvas({
  layer-stack(size: (6, 4), ..sample)
})

#cetz.canvas({
  import cetz.draw: ortho

  ortho({
    layer-stack(size: (6, 4), ..sample)
  })
})
```

`layer-stack` is a CeTZ drawing function. It emits the same 3D geometry in both
cases. Without a projection, CeTZ draws the front view in the canvas plane.
Wrapping it in `ortho` produces an orthographic 3D view.

## Model

### `layer`

```typ
layer(
  thickness: none,
  material: none,
  id: auto,
  label: none,
  style: (:),
)
```

- `thickness` is required and expressed in model units.
- `material` is required and selects a style from the active material palette.
- `id` gives the layer a stable identity for later annotations and operations.
- `label` is Typst content associated with the layer.
- `style` overrides the selected material style for this layer.

### `layer-stack`

```typ
layer-stack(
  size: none,
  ..layers,
)
```

`size` is required and is `(width, depth)` in the same model units as layer
thickness. Layers are ordered from bottom to top. The function draws them
directly into the current CeTZ canvas.

### Camera and Projections

The package uses CeTZ coordinates:

- `x`: width;
- `y`: height;
- `z`: depth.

The plain canvas shows the `x-y` plane as a 2D cross-section. CeTZ's `ortho` and
`perspective` functions provide 3D projections, camera angles, depth sorting,
and face culling. The package does not define another camera or projection API.
