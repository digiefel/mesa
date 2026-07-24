#import "../src/lib.typ" as semi
#import "@preview/cetz:0.5.2": draw

#set page(width: auto, height: auto, margin: 12mm)
#set text(size: 9pt)

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

  draw.on-layer(1, {
    let bottom = "metal.front-right-bottom"
    let top = "metal.front-right-top"

    draw.line(
      (rel: (2, 0, 0), to: bottom),
      (rel: (2, 0, 0), to: top),
      name: "metal-thickness",
      mark: (start: "|", end: "|"),
      stroke: .55pt,
    )
    draw.content(
      "metal-thickness.mid",
      box(
        inset: (x: 2pt, y: .5pt),
        text(size: 8pt)[15 nm],
      ),
      anchor: "west",
    )
  })
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
