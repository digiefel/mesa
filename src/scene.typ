#import "@preview/cetz:0.5.2": draw
#import "kernel.typ" as kernel
#import "polygon.typ" as polygon

#let _validate-volume(volume, index) = {
  assert(
    type(volume) == dictionary,
    message: "volume " + str(index) + " must be a dictionary",
  )
  assert(
    "shapes" in volume,
    message: "volume " + str(index) + " requires shapes",
  )
  assert(
    "bottom" in volume
      and "top" in volume
      and volume.top > volume.bottom,
    message: "volume " + str(index) + " requires top > bottom",
  )
}

#let _covered-top(volumes, volume) = {
  let covered = ()
  for other in volumes {
    if other.bottom == volume.top {
      covered += other.shapes
    }
  }
  covered
}

#let _exposed-top(volumes, volume) = {
  let covered = _covered-top(volumes, volume)
  if covered.len() == 0 {
    volume.shapes
  } else {
    kernel.difference(volume.shapes, covered)
  }
}

#let _render-faces(volumes) = {
  for (index, volume) in volumes.enumerate() {
    _validate-volume(volume, index)
  }

  let ordered = volumes.sorted(key: volume => volume.bottom)
  for (index, volume) in ordered.enumerate() {
    draw.on-layer(index, {
      polygon.extrude(
        volume.shapes,
        bottom: volume.bottom,
        top: volume.top,
        top-shapes: _exposed-top(ordered, volume),
        top-fill: volume.at("top-fill", default: rgb("#b8d6ed")),
        side-fill: volume.at("side-fill", default: rgb("#91b4ce")),
        stroke: none,
        bottom-stroke: none,
      )
    })
  }
}

#let edge-styles = (
  outline: rgb("#263843") + .5pt,
  material: rgb("#263843") + .4pt,
  internal: rgb("#49606e") + .3pt,
)

#let _normal-edge-role(edge) = {
  if edge.visibility == "occluded" or edge.kind == "smooth" {
    none
  } else if edge.kind == "material" {
    "material"
  } else if edge.interior {
    "internal"
  } else {
    "outline"
  }
}

#let _render-edges(volumes, view, styles) = {
  draw.on-layer(100, {
    for edge in kernel.scene-topology(volumes, view) {
      let role = _normal-edge-role(edge)
      if role != none {
        draw.line(
          edge.start,
          edge.end,
          stroke: styles.at(role),
        )
      }
    }
  })
}

#let render(volumes, view: none, styles: edge-styles) = {
  assert(view != none, message: "3D scene rendering requires a view")
  _render-faces(volumes)
  _render-edges(volumes, view, styles)
}

#let render-section(volumes, y) = {
  for (index, volume) in volumes.enumerate() {
    _validate-volume(volume, index)
  }

  for volume in volumes.sorted(key: volume => volume.bottom) {
    let intervals = kernel.cross-section(volume.shapes, y)
    for interval in intervals {
      draw.rect(
        (interval.first(), volume.bottom),
        (interval.last(), volume.top),
        fill: volume.at(
          "section-fill",
          default: volume.at("side-fill", default: rgb("#91b4ce")),
        ),
        stroke: volume.at(
          "stroke",
          default: rgb("#263843") + .5pt,
        ),
      )
    }
  }
}

#let cut-y(volumes, y, keep: "positive") = {
  let result = ()
  for volume in volumes {
    let shapes = kernel.clip-y(volume.shapes, y, keep: keep)
    if shapes.len() > 0 {
      let clipped = volume
      clipped.shapes = shapes
      result.push(clipped)
    }
  }
  result
}

#let cut-line(volumes, line, keep: "left") = {
  assert(
    type(line) == array and line.len() == 2,
    message: "cut line must contain two points",
  )
  let result = ()
  for volume in volumes {
    let shapes = kernel.clip-line(
      volume.shapes,
      line.first(),
      line.last(),
      keep: keep,
    )
    if shapes.len() > 0 {
      let clipped = volume
      clipped.shapes = shapes
      result.push(clipped)
    }
  }
  result
}

#let topology-debug-styles = (
  outline: (
    paint: rgb("#263843"),
    thickness: .8pt,
  ),
  material: (
    paint: rgb("#e98a15"),
    thickness: .6pt,
  ),
  occluded: (
    paint: rgb("#7c3aed"),
    thickness: .45pt,
    dash: "dashed",
  ),
  internal: (
    paint: luma(55%),
    thickness: .35pt,
    dash: "dotted",
  ),
)

#let render-topology-debug(volumes, view: none) = {
  assert(view != none, message: "topology debug rendering requires a view")
  let debug-volumes = volumes.map(volume => {
    let debug-volume = volume
    debug-volume.stroke = none
    debug-volume.top-fill = volume.at(
      "top-fill",
      default: rgb("#b8d6ed"),
    ).transparentize(45%)
    debug-volume.side-fill = volume.at(
      "side-fill",
      default: rgb("#91b4ce"),
    ).transparentize(45%)
    debug-volume
  })
  _render-faces(debug-volumes)

  draw.on-layer(100, {
    for edge in kernel.scene-topology(volumes, view) {
      let role = if edge.visibility == "occluded" {
        "occluded"
      } else if edge.kind == "material" {
        "material"
      } else if edge.interior or edge.kind == "smooth" {
        "internal"
      } else {
        "outline"
      }
      draw.line(
        edge.start,
        edge.end,
        stroke: topology-debug-styles.at(role),
      )
    }
  })
}
