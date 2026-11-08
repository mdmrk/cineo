# 0033. egui 0.36.2 so that fastframe's egui crates can be used

- Status: Accepted
- Date: 2026-11-08

## Context
The owner asked to use fastframe as far as possible, and to lower egui if
that was what it took (2026-11-08). Nine fastframe 0.17.0 crates need egui
0.36; Cineo was on 0.37.0. Research, 2026-11-08:

- VERIFIED: fastframe's workspace pins egui and eframe 0.36.2, on the tag and
  on `main`. No open pull request moves it to 0.37.
- VERIFIED: egui 0.37.0 added colored emoji and system fonts for any script
  on native, kinetic scrolling with rubber-banding, and accessibility
  improvements (invisible widgets hidden from the tree, better menu trees,
  a kittest failure for unnamed inputs).
- VERIFIED by comparing the sources of both versions: Cineo used these 0.37
  APIs: `WidgetInfo` with `Role`, `Ui::scope_id`, `Id::unique`,
  `Response::accessible_name`, `State::clamped_offset` and
  `MouseWheelSource`.
- VERIFIED (`scripts/check.sh`, 2026-11-08): the workspace builds and every
  test that is not `#[ignore]` passes on 0.36.2.
- UNKNOWN: how it looks and behaves on a real desktop.

## Options considered
1. Stay on 0.37 and leave the nine crates unused.
2. Fork fastframe to 0.37: newest egui, but a fork to maintain.
3. Lower Cineo to 0.36.2.

## Decision
- `eframe` and `egui_kittest` are 0.36.2, with the APIs above replaced by
  their 0.36 forms.
- `fastframe-emoji` and `fastframe-fonts` (system fallbacks only, no bundled
  Inter) replace what 0.37 did for emoji and other scripts.
- `fastframe-scroll` gives touchpad glide on Linux, in place of 0.37's
  kinetic scrolling. `line_scroll_speed` stays Cineo's.
- `fastframe-text` applies the desktop's font rendering and follows its
  changes, and on Wayland its text size multiplies Cineo's interface scale.
- `fastframe-graphics` shows a dialog when no OpenGL window can open. Its
  `wgpu` retry stays off, because libmpv renders through OpenGL.
- These run from the real window start only, so the UI tests stay
  independent of the machine's fonts.

## Consequences
- Cineo stays behind egui until fastframe moves (TECHNICAL_DEBT.md).
- The startup scans installed fonts and asks `fc-match` on background
  threads. Reading the desktop's font settings blocks the window start
  briefly.
- The dialog uses Cineo's translations, in the saved language.
- Simplified Chinese, added later, relies on these fallbacks: no CJK font is
  bundled, so a system without one draws it as boxes (TECHNICAL_DEBT.md).
- eframe 0.36 always builds with the clipboard, which 0.37 made optional and
  Cineo had off. Copy and paste should be available (INFERRED, not tried), and two BSL-1.0 crates enter the
  Windows build (LEGAL.md).

## Rejected alternatives
- `icons` and `titlebar`: they need egui's SVG loaders and a new visual
  design, and `titlebar` a new setting.
- `theme`: Cineo has one fixed palette.
- `shell`: its purpose is running without a window, which Cineo does not do.
- A fork (option 2): the owner chose to lower egui.
