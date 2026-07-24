#import "../src/lib.typ" as semi

#set page(width: auto, height: auto, margin: 8mm)
#set text(size: 7pt)

#let sample = {
  import semi: *
  layer("substrate", thickness: 30, material: "substrate", label: [Si])
  layer("dielectric", thickness: 5, material: "dielectric", label: [SiO#sub[2]])
  layer("metal", thickness: 15, material: "metal", label: [Al])
  layer("resist", thickness: 20, material: "resist", label: [resist])
}

#grid(
  columns: 2,
  gutter: 8mm,
  semi.layer-stack(sample),
  semi.layer-stack(
    sample,
    camera: (azimuth: 35deg, elevation: 25deg),
  ),
)
