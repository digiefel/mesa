#import "../src/kernel.typ" as kernel
#import "../src/projection.typ": device-to-cetz
#import "../src/scene.typ" as scene
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
#let cut-y = 2

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

#let cut-scene = scene.cut-y(
  sample-scene,
  cut-y,
  keep: "positive",
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
        (0, cut-y),
        (10, cut-y),
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
  #canvas(length: 5mm, {
    draw.ortho(
      x: 35deg,
      y: 35deg,
      sorted: true,
      cull-face: none,
      {
        draw.transform(device-to-cetz)
        scene.render(sample-scene)
      },
    )
  })
]

#pagebreak()

#align(center)[
  #canvas(length: 5mm, {
    draw.ortho(
      x: 35deg,
      y: 35deg,
      sorted: true,
      cull-face: none,
      {
        draw.transform(device-to-cetz)
        scene.render-topology-debug(sample-scene)
      },
    )
  })
]

#pagebreak()

#align(center)[
  #canvas(length: 5mm, {
    scene.render-section(sample-scene, cut-y)
  })
]

#pagebreak()

#align(center)[
  #canvas(length: 5mm, {
    draw.ortho(
      x: 35deg,
      y: 35deg,
      sorted: true,
      cull-face: none,
      {
        draw.transform(device-to-cetz)
        scene.render(cut-scene)
      },
    )
  })
]
