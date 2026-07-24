# Layer-stack API

How to describe a layer stack and render it in 2D or 3D.

## Scope

Supports:

- horizontal layers with a shared rectangular footprint;
- a thickness, material, and optional label for each layer;
- 2D cross-section and 3D oblique rendering from the same stack;
- package defaults with per-layer style overrides.
- different styles of shading and lighting effects

## Proposed API

```typ
#import "src/lib.typ" as semi

#let sample = {
  import semi: *

  layer(
    "substrate",
    thickness: 1.2,
    material: "silicon",
    label: [Si],
  )
  layer(
    "oxide",
    thickness: 0.18,
    material: "oxide",
    label: [SiO#sub[2]],
  )
  layer(
    "resist",
    thickness: 0.7,
    material: "resist",
    label: [resist],
  )
}

#semi.layer-stack(sample)
// TODO: still not there for the arguments
```

`layer-stack` owns the CeTZ canvas. The functions in its body add layers to the
stack in order. 

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

### `layer-stack`

```typ
layer-stack(
  body,
  size: none,
  ..canvas-arguments,
)
```

`size` is optional and is `(width, depth)` in the same model units as layer
thickness. `layer-stack` initializes the material styles and stack state, then
passes its remaining arguments to `cetz.canvas`.

### Coordinates

The package uses CeTZ coordinates:

- `x`: width;
- `y`: height;
- `z`: depth.

The plain stack shows the `x-y` plane as a 2D cross-section. 
The `z` direction is normal to the substrate plane, i.e. "up".
