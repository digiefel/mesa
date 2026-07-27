#import "@preview/cetz:0.5.2": draw as cetz-draw
#import cetz-draw: *
#import "layer-stack.typ": project-face-content

#let _projection-target(value) = {
  assert(
    type(value) == str,
    message: "project must be a layer face anchor such as \"metal.front\"",
  )
  let parts = value.split(".")
  assert(
    parts.len() == 2
      and parts.at(0) != ""
      and parts.at(1) in ("front", "back", "left", "right", "top", "bottom"),
    message: "project must be a layer face anchor such as \"metal.front\"",
  )
  (layer: parts.at(0), face: parts.at(1))
}

/// CeTZ's content function, with optional projection onto a semiconductor
/// layer face. Placement and projection remain independent: the first
/// coordinate is resolved by CeTZ, while `project` selects the local plane.
#let content(
  ..args-style,
  project: none,
  angle: 0deg,
  anchor: none,
  name: none,
) = {
  if project == none {
    return cetz-draw.content(
      ..args-style,
      angle: angle,
      anchor: anchor,
      name: name,
    )
  }

  let args = args-style.pos()
  let style = args-style.named()
  assert(
    args.len() in (2, 3),
    message: "draw.content expects 2 or 3 positional arguments",
  )
  let projection = _projection-target(project)
  let placement = args.slice(0, args.len() - 1)
  let body = args.last()

  cetz-draw.get-ctx(ctx => {
    let state = ctx.shared-state.at("semi", default: none)
    assert(
      state != none,
      message: "projected draw.content must be used inside layer-stack",
    )
    assert(
      projection.layer in state.layers,
      message: "unknown projection layer: " + repr(projection.layer),
    )
    let body = project-face-content(
      body,
      state.camera,
      projection.face,
      anchor: anchor,
    )
    cetz-draw.content(
      ..placement,
      body,
      ..style,
      angle: angle,
      anchor: anchor,
      name: name,
    )
  })
}
