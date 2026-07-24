#import "fills.typ": hatch, crosshatch, dots

#let edge = rgb("#26343a")

#let default-palette = (
  default: (
    (
      fill: hatch(
        background: rgb("#e7ecee"),
        color: rgb("#aab7bc"),
        spacing: 7pt,
      ),
      stroke: .55pt + edge,
    ),
  ),
  substrate: (
    (
      fill: rgb("#b9cbd0"),
      stroke: .55pt + edge,
      fade-bottom: (
        start: 70%,
        end: 95%,
        color: white,
      ),
    ),
  ),
  dielectric: (
    (
      fill: hatch(
        background: rgb("#ccebf3"),
        color: rgb("#71b8c9"),
        spacing: 6pt,
        thickness: .4pt,
      ),
      stroke: .55pt + edge,
    ),
    (
      fill: hatch(
        background: rgb("#d9d9f2"),
        color: rgb("#9696c8"),
        spacing: 7pt,
        thickness: .4pt,
      ),
      stroke: .55pt + edge,
    ),
    (
      fill: hatch(
        background: rgb("#cce7de"),
        color: rgb("#75ad9a"),
        spacing: 8pt,
        thickness: .4pt,
      ),
      stroke: .55pt + edge,
    ),
  ),
  metal: (
    (
      fill: hatch(
        background: rgb("#e3c66f"),
        color: rgb("#a9852e"),
        spacing: 6pt,
        thickness: .4pt,
        angle: -45deg,
      ),
      stroke: .55pt + edge,
    ),
    (
      fill: hatch(
        background: rgb("#d7a17c"),
        color: rgb("#985f3d"),
        spacing: 7pt,
        thickness: .4pt,
        angle: -45deg,
      ),
      stroke: .55pt + edge,
    ),
    (
      fill: hatch(
        background: rgb("#cbd3d6"),
        color: rgb("#7b8b91"),
        spacing: 8pt,
        thickness: .4pt,
        angle: -45deg,
      ),
      stroke: .55pt + edge,
    ),
  ),
  resist: (
    (
      fill: dots(
        background: rgb("#c9dfa2"),
        color: rgb("#7fa254"),
        spacing: 8pt,
        radius: .6pt,
      ),
      stroke: .55pt + edge,
    ),
    (
      fill: dots(
        background: rgb("#e9b8c5"),
        color: rgb("#b66d82"),
        spacing: 9pt,
        radius: .6pt,
      ),
      stroke: .55pt + edge,
    ),
  ),
)
