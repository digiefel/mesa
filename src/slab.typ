#import "@preview/cetz:0.5.2": draw
#import "kernel.typ" as kernel
#import "polygon.typ" as polygon

#let _validate(slab, index) = {
  assert(
    type(slab) == dictionary,
    message: "slab " + str(index) + " must be a dictionary",
  )
  assert(
    "shapes" in slab,
    message: "slab " + str(index) + " requires shapes",
  )
  assert(
    "bottom" in slab and "top" in slab and slab.top > slab.bottom,
    message: "slab " + str(index) + " requires top > bottom",
  )
}

#let _covered-top(slabs, slab) = {
  let covered = ()
  for other in slabs {
    if other.bottom == slab.top {
      covered += other.shapes
    }
  }
  covered
}

#let _exposed-top(slabs, slab) = {
  let covered = _covered-top(slabs, slab)
  if covered.len() == 0 {
    slab.shapes
  } else {
    kernel.difference(slab.shapes, covered)
  }
}

#let render(slabs) = {
  for (index, slab) in slabs.enumerate() {
    _validate(slab, index)
  }

  let ordered = slabs.sorted(key: slab => slab.bottom)
  for (index, slab) in ordered.enumerate() {
    draw.on-layer(index, {
      polygon.extrude(
        slab.shapes,
        bottom: slab.bottom,
        top: slab.top,
        top-shapes: _exposed-top(ordered, slab),
        top-fill: slab.at("top-fill", default: rgb("#b8d6ed")),
        side-fill: slab.at("side-fill", default: rgb("#91b4ce")),
        stroke: slab.at(
          "stroke",
          default: rgb("#263843") + .5pt,
        ),
      )
    })
  }
}

#let render-section(slabs, y) = {
  for (index, slab) in slabs.enumerate() {
    _validate(slab, index)
  }

  for slab in slabs.sorted(key: slab => slab.bottom) {
    let intervals = kernel.cross-section(slab.shapes, y)
    for interval in intervals {
      draw.rect(
        (interval.first(), slab.bottom),
        (interval.last(), slab.top),
        fill: slab.at(
          "section-fill",
          default: slab.at("side-fill", default: rgb("#91b4ce")),
        ),
        stroke: slab.at(
          "stroke",
          default: rgb("#263843") + .5pt,
        ),
      )
    }
  }
}
