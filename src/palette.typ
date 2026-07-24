#import "fills.typ": hatch, crosshatch, dots

#let edge = rgb("#26343a")

#let default-palette = (
  default: (
    fill: hatch(
      background: rgb("#e7ecee"),
      color: rgb("#aab7bc"),
      spacing: 7pt,
    ),
    stroke: .55pt + edge,
  ),
  substrate: (
    fill: dots(
      background: rgb("#b9cbd0"),
      color: rgb("#718a91"),
      spacing: 7pt,
      radius: .55pt,
    ),
    stroke: .55pt + edge,
  ),
  dielectric: (
    fill: hatch(
      background: rgb("#ccebf3"),
      color: rgb("#71b8c9"),
      spacing: 6pt,
      thickness: .4pt,
    ),
    stroke: .55pt + edge,
  ),
  metal: (
    fill: hatch(
      background: rgb("#e3c66f"),
      color: rgb("#a9852e"),
      spacing: 6pt,
      thickness: .4pt,
      angle: -45deg,
    ),
    stroke: .55pt + edge,
  ),
  resist: (
    fill: crosshatch(
      background: rgb("#f7c978"),
      color: rgb("#d99a42"),
      spacing: 8pt,
      thickness: .35pt,
    ),
    stroke: .55pt + edge,
  ),
)
