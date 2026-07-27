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
    intensity: 0.25,
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
  label-position: (center, horizon),
  bevel: auto,
  internal-stroke: auto,
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
- `label-position` places the label in the face-local `(x, z)` plane.
- `bevel` overrides the stack's bevel configuration for this layer.
- `internal-stroke` overrides the stack's bevel-contour stroke.
- extra named arguments override the selected material style.

```typ
layer("metal-1", thickness: 0.3, material: "metal")
layer("metal-2", thickness: 0.3, material: "metal")
layer("metal-3", thickness: 0.3, material: "metal", variant: 1)
```

The first two layers use successive metal variants. The third explicitly uses
variant 1. It still advances the metal occurrence counter.

### Layer anchors and projected content

```typ
draw.content(
  position,
  body,
  project: none,
  anchor: none,
)
```

The package exports `draw`, a facade over CeTZ's `draw` module. Its functions
are unchanged except where this package explicitly adds semiconductor-aware
behavior.

Every named layer exposes CeTZ anchors for its faces, edges, and corners:

```typ
"metal.front"
"metal.top"
"metal.back-right"
"metal.back-right-bottom"
```

These are ordinary CeTZ coordinates. They work anywhere a CeTZ coordinate is
accepted.

The package's `draw.content` adds one named argument, `project`. It must name
one of the six central face anchors: `front`, `back`, `left`, `right`, `top`,
or `bottom`. Placement remains independent and is still handled entirely by
CeTZ.

```typ
draw.content(
  "resist.front",
  [Photoresist],
  project: "resist.front",
)

draw.content(
  "metal-t.mid",
  text(7pt)[15 nm],
  project: "metal.back",
  anchor: "west",
)
```

In the second call, `"metal-t.mid"` is only the midpoint of a named CeTZ line.
It carries no projection metadata. `"metal.back"` independently supplies the
projection plane.

Layer labels use the same projection implementation. Their `label-position`
components can be alignments, numbers in model units, lengths, ratios, or
relative lengths:

```typ
label-position: (center, horizon)
label-position: (50%, 60%)
label-position: (100% - 2pt, 50% + 1mm)
```

`horizon` is the visual vertical middle. For a material with `fade-bottom`, it
is shifted toward the fully visible part of the layer. An explicit `50%`
always denotes the geometric midpoint.

Material fills can be colors or the package's `hatch`, `crosshatch`, and `dots`
tilings:

```typ
layer(
  "metal",
  thickness: 0.4,
  base-color: rgb("#d8c27a"),
  fill: hatch(
    background: rgb("#d8c27a"),
    color: rgb("#8e762c"),
  ),
)
```

`base-color` is the solid material colour used on bevel faces. This prevents a
tiling from restarting independently on every chamfer. The default material
styles provide it. A custom patterned `fill` should normally provide a matching
`base-color`; a solid-colour `fill` is used automatically.

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
    intensity: 0.25,
  ),
  bevel: (top: 0.5, bottom: 0.25),
  internal-stroke: none,
  palette: (:),
  label-transform: "project",
  length: .8mm,
  baseline: none,
  background: none,
  stroke: none,
  padding: none,
  debug: none,
  canvas-debug: false,
)
```

Model coordinates represent nanometres by default. `size` is `(x-width,
y-depth)` in the same units as layer thickness; its default is `(80, 50)`.

The default camera is the front cross-section. `azimuth` rotates around the
vertical axis; `elevation` moves above the substrate plane. Changing either
angle reveals the depth of the same stack.

`light` uses the same angular convention as `camera`, but its direction is
independent of the camera. The angles define the direction in which the light
travels. "Flat" and "fancy" shading use the same directional Lambertian
calculation and directional self-shadowing for every face:

Azimuth `0deg` travels along `+y`, from the visible front into the sample.
Positive azimuth rotates toward `+x`. Elevation `0deg` lies in the `x-y`
plane, and positive elevation travels toward `-z`, from above toward the
sample.

```text
cosine = max(0, dot(face-normal, -light-direction))
visibility = 0 if the face points away from the light or sample geometry blocks it, otherwise 1
brightness = (1 - intensity) + intensity * visibility * cosine
```

`intensity` accepts a number from `0` to `1` or an equivalent ratio. At `0`,
all face orientations retain the material colour; at `1`, unshadowed points
use the unmodified cosine term and shadowed points receive no direct light.
Changing the camera does not change the light direction, unshadowed
brightness, or model-space shadow geometry.

The light is infinitely far away, so all rays are parallel. Self-shadows are
computed before rendering by projecting every other face along the light
direction, including faces from the same layer. For the current aligned
rectangular stacks, visibility is a single value for each face: if its centre
is blocked, the complete face receives only the ambient term. The face is then
rendered once at that brightness; no shadow overlay is drawn. This face-level
assumption will need to change when the package supports offset or patterned
process geometry.

`shading` accepts `"none"`, `"flat"`, and `"fancy"`. fancy keeps the
material's `fill`, including hatches and dots, but adds one-segment chamfer
faces at exposed layer tops and bottoms. `palette` changes material defaults;
per-layer style arguments still take precedence.

`bevel` controls the chamfer geometry used by fancy shading. A number applies
the same model-space depth at the top and bottom. A ratio is relative to the
layer thickness. A dictionary configures them independently:

```typ
bevel: (top: 0.5, bottom: 0.25)
bevel: (top: 8%, bottom: 4%)
```

The layer-level `bevel` argument overrides this configuration. A fading
substrate has no visible bottom edge, so its bottom bevel is suppressed.

`stroke` in a material or layer style controls the exterior outline.
`internal-stroke` independently controls the contours between bevel faces and
flat faces. It defaults to `none`; `auto` reuses the exterior stroke, and any
CeTZ stroke value can give these contours a lighter or dashed style. A
layer-level `internal-stroke` overrides the stack setting.

`label-transform` controls how labels follow the layer face. `"project"`
applies the face's full orthographic projection, including foreshortening and
shear. `"rotate"` only aligns the baseline with the face, while `"none"` keeps
labels horizontal on the page.

`debug` accepts a CeTZ-style body evaluated after the final stack geometry and
lighting values are known:

```typ
#semi.layer-stack(
  sample,
  debug: {
    import semi.debug: *

    axes()
    light()
    face-info(
      faces: ("front", "right"),
      layers: "resist",
      values: ("cosine", "visibility", "brightness"),
    )
    normals(faces: "top", layers: "resist")
  },
)
```

`light()` draws one ray toward the sample by default, with an open chevron at
its midpoint, and reports azimuth, elevation, and intensity. It also shows the
zero-azimuth reference, the horizontal and vertical projections of the light
direction, and separate azimuth and elevation arcs. Set `angles: false` to
hide that construction or `rays` to draw additional parallel rays.
`axes()` and `light()` share an origin on a model-space sphere centred on the
sample's top face. Its radius is 75% of the sample's three-dimensional
diagonal. The origin depends on the light direction, not the camera, so camera
movement only changes its projection. The light ray points from that origin
toward the sphere centre.
`face-info()` attaches a projected box directly to each selected side face
using the exact normal, cosine, visibility, and brightness values already
computed by the renderer; `normals()` draws the corresponding geometric
normals. By default, `face-info()` selects the visible side faces from the
camera angle. The optional `faces` and `layers` arguments can focus either
helper on specific geometry.

Face names are fixed in model space: `"front"` is `y = 0`, `"back"` is
`y = depth`, `"left"` is `x = 0`, and `"right"` is `x = width`. Camera
rotation never changes those meanings. `faces: auto` only chooses among those
objective names according to which side faces are visible.

`length`, `baseline`, `background`, `stroke`, and `padding` are passed to
`cetz.canvas`. `canvas-debug` separately controls CeTZ's bounding-box debugger.
`length` specifies the rendered length of one model unit and defaults to
`.8mm`. The default front view is 64 mm wide; an oblique view is approximately
half-column width on an A4 page.

The canvas `x`, `y`, and `z` arguments are not exposed. `layer-stack` controls
the coordinate basis to implement device coordinates and the camera.

### Coordinates

The package uses device coordinates:

- `x`: width;
- `y`: depth;
- `z`: height.

The coordinate system is right-handed: `x × y = z`.

The substrate lies in the `x-y` plane. The `z` direction is normal to the
substrate plane, i.e. "up". The default camera shows the `x-z` cross-section.
