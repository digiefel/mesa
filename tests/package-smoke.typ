#import "@preview/mesa:0.1.0" as semi

#set page(width: auto, height: auto, margin: 4mm)

#let sample = {
  import semi: *

  layer("substrate", thickness: 40, material: "substrate")
  layer(
    "oxide",
    thickness: 5,
    material: "dielectric",
    internal-stroke: auto,
  )
  layer("gate", thickness: 15, material: "metal", stroke: none)
}

#semi.layer-stack(
  sample,
  camera: (azimuth: 35deg, elevation: 35deg),
  shading: "fancy",
  stroke: .55pt + black,
  internal-stroke: auto,
  crease-angle: 2deg,
)
