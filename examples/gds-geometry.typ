#import "../src/lib.typ" as semi
#import "@preview/cetz:0.5.2": canvas, draw

#set page(width: auto, height: auto, margin: 12mm)
#set text(size: 9pt, font: "Helvetica")

#let data = read("device.gds", encoding: none)

#let layout = semi.gds(
  data,
  cell: "TOP",
  layers: (
    active: (1, 0),
    gate: (10, 0),
    metal: (20, 0),
  ),
)

#let draw-shapes(shapes, fill) = {
  for shape in shapes {
    draw.compound-path({
      for contour in shape {
        draw.line(..contour, close: true)
      }
    }, fill: fill, fill-rule: "even-odd", stroke: black + .5pt)
  }
}

#let etched-oxide = {
  import semi: *

  layer(
    "substrate",
    thickness: 40,
    material: "substrate",
    label: [Si],
  )
  layer(
    "oxide",
    thickness: 5,
    material: "dielectric",
    mask: mask.invert(layout.metal),
  )
}

#let transistor = {
  import semi: *

  layer(
    "substrate",
    thickness: 40,
    material: "substrate",
    label: [Si],
  )
  layer(
    "oxide",
    thickness: 5,
    material: "dielectric",
    label: [SiO#sub[2]],
    mask: mask.invert(layout.metal),
  )
  layer(
    "gate",
    thickness: 15,
    material: "metal",
    mask: layout.gate,
  )
  layer(
    "metal",
    thickness: 10,
    material: "metal",
    mask: layout.metal,
  )
}

#grid(
  columns: 2,
  gutter: 10mm,
  align: center,
  [#semi.debug.gds(data)],
  [
    #canvas(length: .5mm, {
      draw-shapes(layout.active, rgb("#b8d6ed"))
      draw-shapes(layout.gate, rgb("#ee867f").transparentize(20%))
      draw-shapes(layout.metal, rgb("#e9c46a").transparentize(20%))
    })
  ],
)

#pagebreak()

#let camera = (
  azimuth: 35deg,
  elevation: 35deg,
)

#let light = (
  azimuth: 15deg,
  elevation: 50deg,
  intensity: 0.35,
)

#let section-y = layout.size.at(1) / 2

#grid(
  columns: 3,
  gutter: 12mm,
  align: center,
  [
    #semi.layer-stack(
      etched-oxide,
      size: layout.size,
      camera: camera,
      shading: "fancy",
      light: light,
      length: .5mm,
    )
  ],
  [
    #semi.layer-stack(
      transistor,
      size: layout.size,
      camera: camera,
      shading: "fancy",
      light: light,
      length: .5mm,
    )
  ],
  [
    #semi.layer-stack(
      transistor,
      size: layout.size,
      section: ((0, section-y), (layout.size.at(0), section-y)),
      length: .5mm,
    )
  ],
)
