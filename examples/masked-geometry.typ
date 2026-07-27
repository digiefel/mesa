#import "../src/kernel.typ" as kernel
#import "../src/projection.typ": device-to-cetz, ortho-view
#import "../src/scene.typ" as scene
#import "../src/lib.typ" as semi
#import "@preview/cetz:0.5.2": canvas, draw

#set page(width: auto, height: auto, margin: 12mm)
#set text(size: 9pt, font: "Helvetica")

#let subject = (
  (
    ((0, 0), (10, 0), (10, 6), (0, 6)),
  ),
)

#let mask = (
  (
    ((2, 1), (4, 1), (4, 3), (2, 3)),
  ),
  (
    ((6, -1), (7, -1), (7, 7), (6, 7)),
  ),
)

#let result = kernel.difference(subject, mask)
#let section-y = 2
#let cut-line = ((0, 2), (10, 4))
#let view-x = 35deg
#let view-y = 35deg
#let view-z = 0deg
#let view = ortho-view(view-x, view-y, z: view-z)

#let masked-sample = {
  import semi: *

  layer(
    "substrate",
    thickness: 1.5,
    material: "substrate",
    fade-bottom: none,
  )
  layer(
    "pattern",
    thickness: 1.5,
    material: "dielectric",
    mask: result,
  )
}

#let sample-scene = (
  (
    shapes: subject,
    bottom: -1.5,
    top: 0,
    top-fill: rgb("#c6d1d6"),
    side-fill: rgb("#9fadb4"),
    section-fill: rgb("#9fadb4"),
  ),
  (
    shapes: result,
    bottom: 0,
    top: 1.5,
    section-fill: rgb("#b8d6ed"),
  ),
)

#let cut-scene = scene.cut-line(
  sample-scene,
  cut-line,
  keep: "left",
)

#let draw-shapes(shapes, fill: none, stroke: black + .5pt) = {
  for shape in shapes {
    draw.compound-path({
      for contour in shape {
        draw.line(..contour, close: true)
      }
    }, fill: fill, fill-rule: "even-odd", stroke: stroke)
  }
}

#let geometry-view(body) = canvas(length: 5mm, {
  body
})

#grid(
  columns: 3,
  gutter: 8mm,
  align: center,
  [
    #geometry-view({
      draw-shapes(subject, fill: rgb("#b8d6ed"))
    })
  ],
  [
    #geometry-view({
      draw-shapes(subject, fill: rgb("#b8d6ed"))
      draw-shapes(mask, fill: rgb("#ee867f").transparentize(25%))
    })
  ],
  [
    #geometry-view({
      draw-shapes(result, fill: rgb("#b8d6ed"))
      draw.line(
        ..cut-line,
        stroke: (paint: rgb("#d1495b"), thickness: .5pt, dash: "dashed"),
      )
    })
  ],
)

#pagebreak()

#align(right, text(size: 7pt, fill: luma(45%))[
  geometry kernel #kernel.version()
])

#align(center)[
  #semi.layer-stack(
    masked-sample,
    size: (10, 6),
    camera: (
      azimuth: view-y,
      elevation: view-x,
    ),
    length: 5mm,
  )
]

#pagebreak()

#align(center)[
  #grid(
    columns: 4,
    gutter: 5mm,
    [#line(length: 6mm, stroke: scene.topology-debug-styles.outline) outline],
    [#line(length: 6mm, stroke: scene.topology-debug-styles.material) material],
    [#line(length: 6mm, stroke: scene.topology-debug-styles.occluded) occluded],
    [#line(length: 6mm, stroke: scene.topology-debug-styles.internal) internal],
  )
  #v(3mm)
  #canvas(length: 5mm, {
    draw.ortho(
      x: view-x,
      y: view-y,
      z: view-z,
      sorted: true,
      cull-face: none,
      {
        draw.transform(device-to-cetz)
        scene.render-topology-debug(sample-scene, view: view)
      },
    )
  })
]

#pagebreak()

#align(center)[
  #canvas(length: 5mm, {
    scene.render-section(sample-scene, section-y)
  })
]

#pagebreak()

#align(center)[
  #canvas(length: 5mm, {
    draw.ortho(
      x: view-x,
      y: view-y,
      z: view-z,
      sorted: true,
      cull-face: none,
      {
        draw.transform(device-to-cetz)
        scene.render(cut-scene, view: view)
      },
    )
  })
]
