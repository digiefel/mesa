# Mesa

Draw 2D and 3D semiconductor devices and fabrication processes with
[CeTZ](https://github.com/cetz-package/cetz), including patterned geometry
loaded from GDS.

```typ
#import "@preview/mesa:0.1.0" as semi

#set page(width: auto, height: auto, margin: 8mm)

#let sample = {
  import semi: *

  layer(
    "substrate",
    thickness: 30,
    material: "substrate",
    label: [Si],
  )
  layer(
    "oxide",
    thickness: 5,
    material: "dielectric",
    label: [SiO#sub[2]],
  )
  layer(
    "gate",
    thickness: 15,
    material: "metal",
    label: [Al],
  )
}

#semi.layer-stack(
  sample,
  camera: (azimuth: 35deg, elevation: 35deg),
  shading: "fancy",
)
```

![A layer stack rendered with Mesa](assets/layer-stack.png)

GDS layouts can also be used to produce process-flow steps.

![A transistor fabrication process rendered with Mesa](assets/process-flow.png)

> DISCLAIMER: >80% of Typst code and 100% of Rust code was LLM-written, guided by me at a high level. I'm not fluent in Rust and I couldn't have justified writing this package otherwise. Since this is only a visualiation tool, I felt that vibe coding it would be justified.

In case of any bugs or missing features, please [open an issue](https://github.com/digiefel/mesa/issues) on GitHub.
