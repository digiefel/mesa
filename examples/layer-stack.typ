#import "../src/lib.typ" as semi
#import "@preview/cetz:0.5.2": draw

#set page(width: auto, height: auto, margin: 12mm)
#set text(size: 9pt, font: "Helvetica")

#let sample = {
  import semi: *

  layer(
    "substrate",
    thickness: 20,
    material: "substrate",
    label: [Si],
  )
  layer(
    "dielectric",
    thickness: 5,
    material: "dielectric",
    label: [SiO#sub[2]],
  )
  layer(
    "metal",
    thickness: 15,
    material: "metal",
    label: [Al],
  )
  layer(
    "resist",
    thickness: 20,
    material: "resist",
    label: [Photoresist],
  )

  // an annotation
  draw.set-style(content: (padding: 2pt))
  draw.line(
    (rel: (2, 0), to: "metal.front-right-bottom"),
    (rel: (2, 0), to: "metal.front-right-top"),
    name: "metal-t",
    mark: (start: "|", end: "|"),
    stroke: .55pt,
  )
  draw.content("metal-t.mid", text(7pt)[15 nm], anchor: "west")
}

#grid(
  columns: 2,
  gutter: 12mm,
  align: center,
  [
    #align(center)[
      #semi.layer-stack(
        sample,
      )
    ]
  ],
  [
    #align(center)[
      #semi.layer-stack(
        sample,
        label-transform: "project",
        camera: (
          azimuth: 35deg,
          elevation: 25deg,
        ),
        light: (
          azimuth: -45deg,
          elevation: 60deg,
        ),
        shading: "flat",
      )
    ]
  ],
)
