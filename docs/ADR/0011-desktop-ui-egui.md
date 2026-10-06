# 0011. Desktop UI: egui (eframe, glow backend) with a custom theme

- Status: Accepted
- Date: 2026-10-06
- Supersedes: [0005](0005-desktop-ui-tauri.md)

## Context
The owner decided on 2026-10-06 to build the desktop UI with **egui** and
a custom-designed interface. This replaces the proposed Tauri design
(ADR-0005). That ADR had listed egui as its fallback because of its single
language, simplicity and agent-friendliness.

Research, crates.io 2026-10-06:
- `eframe` and `egui_extras` 0.36.2 are MIT OR Apache-2.0, with MSRV 1.95
  (below our pin of 1.99).
- eframe offers a `wgpu` backend (the default) and a `glow` (OpenGL)
  backend.
- `egui_extras`' `http` image loader uses its own HTTP client (`ehttp`),
  which would bypass `NetPolicy`.

## Decision
- The `cineo-desktop` crate uses `eframe` with **default features off**,
  plus `glow`, `default_fonts`, `x11`, `wayland` and `accesskit`.
  - glow over wgpu: smaller dependency tree and faster builds.
  - OpenGL is also what libmpv's render API targets, which keeps the future
    embedded-player path (ADR-0004 follow-up) open. (INFERRED: to be
    verified when embedding is attempted.)
- **Images** go through `cineo-net`. A custom egui `BytesLoader` fetches
  `http(s)` URIs with the same network policy, plus an image-specific size
  limit. `egui_extras` (`image` feature only) and `image` (jpeg, png and
  webp only) decode the bytes. The `http` and `file` loaders are not
  enabled.
- **Look.** A custom dark theme lives in one `theme` module: palette,
  spacing, rounding, typography scale. It has a top navigation bar, poster
  cards with hover states, and a backdrop hero on the detail page. Widgets
  read the theme; they never hardcode colors.
- **State flow (ADR-0001).** The egui app owns the state. Each frame it
  drains a channel of IO results, applies them, and renders. User actions
  spawn IO on a tokio runtime owned by the app. Results come back through
  the channel and the app requests a repaint. Rendering code performs no IO.

## Consequences
- Single language, no JS toolchain, simpler CI.
- 10-foot polish and accessibility need deliberate work. AccessKit is
  enabled.
- The immediate-mode UI makes stale-request handling explicit. Every
  result carries the request key it answers, so stale results can be
  dropped.
- ADR-0005's Tauri-specific security items (CSP, webview) no longer apply.
  Image fetching is now under our network policy, which is stricter than a
  webview.

## Rejected alternatives
- Tauri (ADR-0005), superseded by the owner's decision.
- The eframe wgpu backend: a heavier build with no benefit for a 2D UI;
  it can be revisited if rendering needs it.
- The egui_extras `http` loader: it bypasses the network policy.
