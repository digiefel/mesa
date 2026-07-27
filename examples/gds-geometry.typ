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

#let substrate = {
  import semi: *

  layer(
    "substrate",
    thickness: 40,
    material: "substrate",
    label: [Si],
  )
}

#let oxidized = {
  import semi: *

  substrate

  layer(
    "oxide",
    thickness: 5,
    material: "dielectric",
  )
}

#let gated = {
  import semi: *

  oxidized
  layer(
    "gate",
    thickness: 15,
    material: "metal",
    mask: layout.gate,
  )
}

#let etched = {
  import semi: *

  gated
  etch(depth: 5, mask: layout.metal)
}

#let contacted = {
  import semi: *

  etched
  layer(
    "metal",
    thickness: 10,
    material: "metal",
    mask: layout.metal,
  )
}

#grid(
  columns: (9cm, auto),
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

#let step(name, body, section: none, length: .35mm) = block(
  width: 50mm,
  height: 46mm,
  inset: 3mm,
  radius: 2pt,
  fill: luma(98%),
  stroke: luma(78%) + .5pt,
)[
  #grid(
    rows: (1fr, auto),
    row-gutter: 1.5mm,
    align: center + horizon,
    [
      #semi.layer-stack(
        body,
        size: layout.size,
        camera: camera,
        shading: "fancy",
        light: light,
        section: section,
        length: length,
      )
    ],
    text(8pt, weight: "medium", name),
  )
]

#grid(
  columns: 3,
  gutter: 8mm,
  row-gutter: 7mm,
  align: center,
  step([Substrate], substrate),
  step([SiO#sub[2]], oxidized),
  step([Gate], gated),
  step([Etch], etched),
  step([Metal contacts], contacted),
  step(
    [Cross section],
    contacted,
    section: ((0, section-y), (layout.size.at(0), section-y)),
  ),
)

#pagebreak()

#let gate-resist = {
  import semi: *

  oxidized
  layer(
    "gate-resist",
    thickness: 12,
    material: "resist",
    mask: mask.invert(layout.gate),
  )
}

#let gate-metal = {
  import semi: *

  gate-resist
  layer(
    "gate-metal",
    thickness: 15,
    material: "metal",
    mask: layout.gate,
  )
}

#let contact-resist = {
  import semi: *

  gated
  layer(
    "contact-resist",
    thickness: 12,
    material: "resist",
    mask: mask.invert(layout.metal),
  )
}

#let resist-etched = {
  import semi: *

  contact-resist
  etch(depth: 5, mask: layout.metal)
}

#let blanket-metal = {
  import semi: *

  resist-etched
  layer(
    "contact-metal",
    thickness: 10,
    material: "metal",
  )
}

#let lifted-off = {
  import semi: *

  contacted
}

#grid(
  columns: 4,
  gutter: 5mm,
  row-gutter: 5mm,
  align: center,
  step([Substrate], substrate, length: .26mm),
  step([SiO#sub[2]], oxidized, length: .26mm),
  step([Gate resist], gate-resist, length: .26mm),
  step([Gate metal], gate-metal, length: .26mm),
  step([Contact resist], contact-resist, length: .26mm),
  step([Etch], resist-etched, length: .26mm),
  step([Blanket metal], blanket-metal, length: .26mm),
  step([Lift-off], lifted-off, length: .26mm),
)
