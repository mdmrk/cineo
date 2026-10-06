# 0005. Desktop UI: Tauri 2 + web frontend

- Status: Superseded by [0011](0011-desktop-ui-egui.md)
- Date: 2026-10-06

## Context
The UI is a 10-foot-friendly media browser: poster grids, images, smooth scrolling, rich styling. Video is handled by mpv (ADR-0004), so the UI toolkit does not need to render video for the MVP. Candidates researched (crates.io, 2026-10-06): tauri 2.12 (MIT/Apache), wry 0.57, egui 0.36 (MIT/Apache), iced 0.14 (MIT; last release 2025-12), slint 1.18 (GPL-3.0 or proprietary licenses).

| Option | For | Against |
|--------|-----|---------|
| A/F. Rust core + web UI (Tauri 2) | Best styling/layout tooling; well known to agents; Tauri 2 also targets Android/iOS; IPC is a natural action/state boundary | Second language + JS toolchain; WebKitGTK performance on Linux; system webview differences |
| B. Core compiled to WASM + web UI | Stremio-web model | We have no browser target; WASM constraints (no `Send`, no sockets) for no benefit |
| D. Raw wry/tao | Fewer layers | Rebuilds what Tauri provides (IPC, packaging, CSP) |
| E. egui | Single language, simple, agent-friendly | Immediate-mode look, weaker accessibility, 10-foot polish is hard |
| E. iced | Elm architecture matches ADR-0001 | API churn, slower releases |
| E. Slint | Polished, declarative | Licensing (GPL or proprietary) conflicts with MIT distribution |
| G. Local web server + browser | Zero GUI deps | Poor desktop integration; localhost attack surface |
| H. Native shell + embedded web + mpv in-window (shell-ng model) | Best playback integration | Per-OS windowing work; shell-ng is Windows-only |

## Decision (proposed)
Tauri 2 with a TypeScript web frontend (framework decided in the spike, preferring minimal dependencies), mpv as external window (ADR-0004). The UI only renders state and sends typed actions; all logic stays in Rust.

Spike exit criteria (all must pass on Linux and Windows): a 500-poster grid scrolls smoothly on WebKitGTK; image loading respects the network policy (images fetched via `cineo-net`, not directly by the webview, or a documented alternative); mpv launch/focus handoff works; strict CSP with no remote scripts.

## Consequences
- If accepted: adds a `ui/` frontend workspace and a JS toolchain to CI in M5.
- If the spike fails: fall back to egui (single language), recorded in a new ADR.
- Until M5, the CLI is the only shell, which keeps the core honest.

## Rejected alternatives
B, D, G, H for the reasons in the table; Slint for licensing.
