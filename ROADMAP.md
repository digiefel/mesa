# Roadmap

The next stage of the package is a constrained 2.5D geometry model for
semiconductor process illustrations. It will support arbitrary polygonal
footprints while retaining horizontal surfaces and vertical sidewalls.

This is a visualization model, not a process simulator or a general-purpose
solid modeller.

## Geometry model

A device is represented as a sequence of horizontal slabs. Within each slab,
materials occupy non-overlapping multipolygon regions in the `(x, y)` plane;
void is implicit.

This model supports:

- arbitrary polygonal footprints;
- concave regions, holes, and disconnected islands;
- trenches, vias, cavities, mesas, and suspended rectangular structures;
- vertical material interfaces and sidewalls;
- exact reuse of the current rectangular layer-stack model as its simplest
  case.

Curved surfaces, sloped sidewalls, physical process simulation, and general
3D meshes are outside the initial scope.

## Geometry kernel

Polygon operations will be implemented in a small Rust library compiled to
WebAssembly. Typst remains responsible for the public API, process
composition, material styling, annotations, and CeTZ rendering.

The kernel will use
[`i_overlay`](https://docs.rs/i_overlay/latest/i_overlay/) directly.

`i_overlay` is preferred over `geo` because the package needs a focused layout
geometry kernel with explicit fixed-grid arithmetic. `geo` provides a much
broader geospatial API, exposes floating-point boolean operations, and already
uses `i_overlay` for those operations. Using `i_overlay` directly keeps the
precision model and geometry representation under the package's control.

The kernel will:

- use integer coordinates on a documented fixed grid;
- normalize polygon orientation and ordering after every operation;
- support union, intersection, difference, and exclusive-or;
- preserve holes and disconnected components;
- simplify redundant vertices and reject or normalize invalid rings;
- expose a pure, versioned byte interface suitable for Typst plugins.

Typst and the plugin will exchange compact scene and operation data. The
plugin will not own rendering state or package-level material semantics.

## Rendering

Rendering will be derived from the slab model:

- horizontal faces come from material regions at slab boundaries;
- vertical faces come from exposed polygon edges between adjacent boundaries;
- compound CeTZ paths represent holes without triangulation;
- existing projection, lighting, bevel, pattern, outline, label, and
  annotation code remains responsible for presentation.

This separates geometric correctness from visual styling and avoids using
rendered CeTZ paths as the source of truth.

## Implementation stages

### 1. Polygonal layers

- Add polygon and multipolygon footprints to layers.
- Keep the current rectangular footprint as the default.
- Render concave footprints, holes, and disconnected regions in both 2D and
  3D.
- Define stable anchors for polygonal layers and their exposed faces.

This stage establishes the public geometry vocabulary without process
operations.

### 2. Rust/WASM geometry kernel

- Add the Rust crate and reproducible WASM build.
- Define the fixed-grid precision and versioned interchange format.
- Implement canonical polygon conversion and boolean operations.
- Cover rectangles, concave polygons, holes, islands, touching edges, and
  degenerate input with geometry tests.

### 3. Slab scene model

- Replace the renderer's implicit rectangular stack with explicit
  `z` boundaries and material regions.
- Partition overlapping additions into non-overlapping material regions.
- Derive exposed horizontal and vertical faces from adjacent slabs.
- Preserve the existing `layer(...)` composition as a compatibility layer.

At this point, successive figures can still be described by ordinary Typst
composition while sharing and extending earlier device bodies.

### 4. Patterned additions and directional deposition

- Add material over a specified footprint and vertical interval.
- Deposit vertically onto upward-facing exposed regions.
- Support thickness, material selection, and a mask or opening polygon.
- Merge the result back into canonical slabs.

This covers lithographically patterned rectangular and polygonal additions
without pretending to simulate transport or growth physics.

### 5. Directional etching

- Remove material vertically through a mask or opening polygon.
- Support a fixed depth, selected target materials, and material or interface
  stop conditions.
- Produce trenches, vias, separated mesas, and released regions where the
  constrained geometry permits them.

### 6. Orthogonal conformal deposition

- Coat exposed horizontal and vertical surfaces.
- Use square or mitered polygon offsets consistent with the vertical-sidewall
  geometry model.
- Resolve overlap between the deposited film and existing materials through
  boolean operations.

The result is a schematic orthogonal approximation. Exact conformal growth
would round corners and is outside the selected geometry scope.

### 7. Orthogonal conformal etching

- Remove material inward from exterior-accessible exposed surfaces.
- Track connected exterior void so sealed cavities are not etched.
- Support material selection, etch distance, and stop conditions.

This is the most demanding operation and follows only after exposed-face and
void-connectivity handling are reliable.

### 8. Layout import

- Accept GDS bytes through the Typst plugin boundary.
- Select layers and datatypes and map them to package materials or operations.
- Flatten selected cells and instances into polygonal footprints.
- Support coordinate scaling, cropping, and simplification.

The importer will not be a layout editor, DRC engine, or full GDS processing
environment.

## Validation

Geometry tests will verify:

- valid, canonical rings and hole assignment;
- non-overlapping material regions within each slab;
- stable results at the configured precision;
- area conservation for boolean operations;
- correct exposed-face extraction;
- exterior-void connectivity for conformal etching.

Typst examples will cover the same fixtures in 2D and 3D, followed by
representative structures from the inspiration set: patterned stacks,
trenches, vias, mesas, suspended layers, directional processes, and
orthogonal conformal coatings.
