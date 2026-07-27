#import "../src/lib.typ" as semi
#import "../src/kernel.typ" as kernel
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
    })
  ],
)

#pagebreak()

#align(right, text(size: 7pt, fill: luma(45%))[
  geometry kernel #kernel.version()
])

#let sample = {
  import semi: *

  layer(
    "substrate",
    thickness: 20,
    material: "substrate",
    label: [Si],
    fade-bottom: (
      start: 50%,
      end: 99%,
      color: white,
    ),
  )
}

#grid(
  columns: 2,
  gutter: 12mm,
  align: center,
  [
    #align(center)[
      #semi.layer-stack(sample)
    ]
  ],
  [
    #align(center)[
      #semi.layer-stack(
        sample,
        label-transform: "project",
        camera: (
          azimuth: 35deg,
          elevation: 35deg,
        ),
      )
    ]
  ],
)
