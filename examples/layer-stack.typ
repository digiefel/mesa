#import "../src/lib.typ" as semi
#import "@preview/cetz:0.5.2": draw

#set page(width: auto, height: auto, margin: 12mm)
#set text(size: 9pt, font: "Helvetica")

#let sample-light = (
  azimuth: 25deg,
  elevation: 55deg,
  intensity: 0.25,
)

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
  )
  face-content(
    "resist",
    [Photoresist],
    position: (center, horizon),
  )

  // an annotation
  draw.set-style(content: (padding: 2pt))
  draw.line(
    (rel: (1.5, 0), to: "metal.back-right-bottom"),
    (rel: (1.5, 0), to: "metal.back-right-top"),
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
        light: sample-light,
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
          elevation: 35deg,
        ),
        light: sample-light,
        shading: "flat",
      )
    ]
  ],
)

#pagebreak()

#grid(
  columns: 2,
  gutter: 12mm,
  align: center,
  [
    #align(center)[
      #semi.layer-stack(
        sample,
        light: sample-light,
        shading: "fancy",
        bevel: (top: 0.5, bottom: 0.25),
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
          elevation: 35deg,
        ),
        light: sample-light,
        shading: "fancy",
      )
    ]
  ],
)

#pagebreak()

#align(center)[
  #semi.layer-stack(
    sample,
    label-transform: "project",
    camera: (
      azimuth: 35deg,
      elevation: 35deg,
    ),
    light: sample-light,
    shading: "fancy",
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
]
