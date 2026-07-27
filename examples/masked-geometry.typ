#import "../src/lib.typ" as semi
#import "../src/kernel.typ" as kernel

#set page(width: auto, height: auto, margin: 12mm)
#set text(size: 9pt, font: "Helvetica")

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
