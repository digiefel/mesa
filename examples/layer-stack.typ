#import "../src/lib.typ" as semi

#set page(width: auto, height: auto, margin: 12mm)
#set text(size: 7pt)

#let sample = {
  import semi: *

  layer(
    "substrate",
    thickness: 1.2,
    material: "substrate",
    label: [substrate],
  )
  layer(
    "dielectric",
    thickness: 0.45,
    material: "dielectric",
    label: [dielectric],
  )
  layer(
    "metal",
    thickness: 0.25,
    material: "metal",
    label: [metal],
  )
  layer(
    "resist",
    thickness: 0.7,
    material: "resist",
    label: [resist],
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
        size: (6, 4),
        length: 6mm,
      )
    ]
  ],
  [
    #align(center)[
      #semi.layer-stack(
        sample,
        size: (6, 4),
        camera: (
          azimuth: 35deg,
          elevation: 25deg,
        ),
        light: (
          azimuth: -45deg,
          elevation: 60deg,
        ),
        shading: "flat",
        length: 6mm,
      )
    ]
  ],
)
