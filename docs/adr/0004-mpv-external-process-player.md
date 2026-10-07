# 0004. mpv as an external process over JSON IPC

- Status: Superseded by 0014
- Date: 2026-10-06

## Context
Addon streams are commonly MKV/HEVC/AC3 with ASS subtitles. Webview `<video>` cannot play most of these, especially WebKitGTK on Linux (INFERRED from codec support of platform webviews). mpv plays them with hardware decoding and libass. stremio-shell-ng embeds libmpv and lets mpv render directly into the native window for performance (VERIFIED-REF, README). Embedding requires FFI (`unsafe`), per-platform window handling, and links LGPL-2.1 libmpv.

## Options considered
1. HTML5 video in a webview.
2. libmpv embedded (render API or `wid`).
3. mpv as a separate process controlled via `--input-ipc-server` JSON IPC.
4. GStreamer / ffmpeg-based custom player.

## Decision
Option 3 for the MVP. Cineo spawns mpv with a controlled option set (`--no-config`, `--ytdl=no`, `--idle`, IPC socket in a private directory), sends typed commands (`loadfile` with JSON arguments, `set_property`, `observe_property`) and consumes events. The core defines `PlayerCommand`/`PlayerEvent`; no raw mpv commands cross the boundary.

## Consequences
- Full codec/HW-decode/subtitle support from day one, on all desktop OSes, with no `unsafe` and no LGPL linking.
- Playback appears in a separate mpv window (acceptable for MVP; embedded playback is future work behind the same boundary).
- mpv must be installed or bundled (packaging, M7). Behavior depends on mpv version — log it at startup.

## Rejected alternatives
- Option 1: codec coverage. Option 2: deferred, not rejected — revisit after MVP via a new ADR. Option 4: reimplementing a player is out of scope.
