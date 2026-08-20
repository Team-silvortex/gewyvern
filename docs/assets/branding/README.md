# Branding Assets

This directory keeps the current `gewyvern` brand assets for the `v1.15.x`
line.

## Current Files

- [gewyvern-logo-v1.svg](gewyvern-logo-v1.svg)
  Preferred master logo for docs, web surfaces, and future exports.
- [gewyvern-logo-v1.png](gewyvern-logo-v1.png)
  Raster export kept for compatibility with surfaces that still want PNG.

## Visual Direction

- motif:
  compact wyvern mark inside a hexagonal shield-like frame
- primary colors:
  gold `#e0b53a`
  orange `#f59d0c`
- style:
  minimal, vector-first, disciplined, battle-ready, CLI/tooling friendly

## Usage Guidance

- prefer the SVG asset for README, docs, web headers, and favicon-style reuse
- use the PNG asset only when a target surface cannot render SVG cleanly
- keep generous empty space around the mark
- do not add extra gradients, glows, or text directly into the master asset
- if a future variant is needed, add a new file instead of mutating the current
  `v1` master silently

## Current Integration Points

- [README.md](../../../README.md)
- [docs/index.md](../../index.md)
- [docs/book/index.md](../../book/index.md)
- [apps/leserpent/src/Leserpent/wwwroot/index.html](../../../apps/leserpent/src/Leserpent/wwwroot/index.html)
