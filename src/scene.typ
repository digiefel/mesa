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

#let render(volumes) = {
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
        stroke: volume.at(
          "stroke",
          default: rgb("#263843") + .5pt,
        ),
      )
    })
  }
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
