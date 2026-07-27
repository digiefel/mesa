#import "@preview/mesa:0.1.0" as semi

#set page(width: auto, height: auto, margin: 4mm)

#let sample = {
  import semi: *

  layer("substrate", thickness: 40, material: "substrate")
  layer("oxide", thickness: 5, material: "dielectric")
  layer("gate", thickness: 15, material: "metal")
}

#semi.layer-stack(
  sample,
  camera: (azimuth: 35deg, elevation: 35deg),
  shading: "fancy",
)
