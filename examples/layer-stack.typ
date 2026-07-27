#import "../src/lib.typ" as semi

#set page(width: auto, height: auto, margin: 12mm)
#set text(size: 9pt, font: "Helvetica")

#let sample-light = (
  azimuth: 15deg,
  elevation: 50deg,
  intensity: 0.35,
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
  draw.content(
    "resist.front",
    [Photoresist],
    project: "resist.front",
  )

  // an annotation
  draw.set-style(content: (padding: 2pt))
  draw.line(
    (rel: (1.5, 0), to: "metal.back-right-bottom"),
    (rel: (1.5, 0), to: "metal.back-right-top"),
    name: "metal-t",
    mark: (start: "|", end: "|", transform-shape: true),
    stroke: .55pt,
  )
  draw.content(
    "metal-t.mid",
    text(7pt)[15 nm],
    project: "metal.back",
    anchor: "west",
  )
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
