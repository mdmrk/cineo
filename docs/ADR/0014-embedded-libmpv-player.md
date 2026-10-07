# 0014. Embedded playback with libmpv, loaded at runtime

- Status: Accepted
- Date: 2026-10-07
- Supersedes: 0004

## Context
ADR-0004 runs mpv as a separate process over JSON IPC. Playback opens a
second window, and users must install mpv. It deferred libmpv embedding to
"a new ADR after MVP". The owner asked for embedded playback on
2026-10-07.

What we checked:
- libmpv's client and render headers (`client.h`, `render.h`,
  `render_gl.h`) are ISC-licensed. libmpv itself is LGPL-2.1+ or GPL-2.0+,
  depending on how it was built (VERIFIED: headers vendored in
  `libmpv2-sys` 4.0.1).
- `libmpv2` 6.0.0 and `libmpv2-sys` 4.0.1 (crates.io, 2026-05) are the
  maintained Rust bindings. Both are **LGPL-2.1 Rust code**, which would be
  compiled statically into our MIT binary. `libmpv2-sys` links `mpv` at
  build time (`cargo:rustc-link-lib=mpv`), so the binary would not start
  without libmpv (VERIFIED: build.rs).
- The render API's OpenGL backend needs a `get_proc_address` function and a
  target framebuffer. eframe 0.36 (glow) exposes
  `CreationContext::get_proc_address` and `egui_glow::CallbackFn`. The
  callback draws into the screen framebuffer (`intermediate_fbo()` returns
  `None`) (VERIFIED: eframe 0.36.2 and egui_glow 0.36.2 sources). ADR-0011
  chose glow partly for this reason.
- `egui-sharkplayer` 0.5.1 (Unlicense) shows the same approach working
  with eframe and glow. It depends on `libmpv2`, so it has the same
  licensing and linking problems (VERIFIED: crates.io metadata).
- `libloading` 0.9.0 (ISC) loads shared libraries at runtime. 0.8.9 is
  already in our tree as a transitive dependency (VERIFIED: crates.io,
  `~/.cargo/registry`).
- mpv 0.41 / libmpv client API 2.5 is installed on the development machine
  (VERIFIED: `pkg-config --modversion mpv`).

## Options considered
1. **Keep the external process** (ADR-0004). No `unsafe`, but no embedding.
2. **`libmpv2` crate.** Ready-made and tested. It brings LGPL Rust code into
   our static binary, so we would have to provide relinkable objects. It
   also adds a hard link-time dependency on libmpv, in the build and at
   runtime.
3. **Our own minimal FFI, written from the ISC headers, with libmpv linked
   at build time.** MIT code. Every builder and CI job needs libmpv's
   development files, and the app does not start without libmpv.
4. **Our own minimal FFI, with libmpv loaded at runtime via `libloading`.**
   MIT code, and builds and CI need nothing extra. A missing libmpv becomes
   an error we can handle: fall back to the external process. The cost is
   an `unsafe` boundary that we maintain, and resolving symbols by name.
5. **`--wid` embedding** (mpv draws into a native child window). Wayland
   has no foreign child windows, and winit does not create them for us.

## Decision
Option 4.

- **Where.** A new `embedded` module in `cineo-player-mpv`. It is the same
  IO boundary (the mpv player), so no new crate is needed (ADR-0002). The
  crate's lint becomes `unsafe_code = "deny"`. Only the FFI module
  (`embedded::ffi`) and the render module have `#[allow(unsafe_code)]`,
  and every `unsafe` block has a `// SAFETY:` comment.
- **Bindings.** Hand-written `extern "C"` types and function pointers for
  only the functions we call. They are resolved with `libloading` 0.8 (the
  version already in our tree through glutin, so no duplicate) and
  checked against `mpv_client_api_version()`: client API major version 2,
  minimum mpv 0.35. Library lookup order: next to the executable first
  (bundling, M7), then the system names (`libmpv.so.2`, `libmpv.so`,
  `libmpv.2.dylib`, `libmpv-2.dll`, `mpv-2.dll`).
- **Safety settings, the same as ADR-0004.** Before `mpv_initialize` we set
  `config=no`, `ytdl=no`, `load-scripts=no`, `osc=no`, `terminal=no`,
  `input-default-bindings=no`, `input-vo-keyboard=no`, `hwdec=auto-safe`
  and `vo=libmpv`. There is no IPC server. Media is loaded with
  `mpv_command` (an argv array) and never with `mpv_command_string`. The
  title goes through `force-media-title`, headers through
  `http-header-fields` (re-validated), and resume is a `seek` after
  `file-loaded`.
- **Rendering.** The desktop shell creates the render context on the UI/GL
  thread from eframe's `get_proc_address`. It draws in an
  `egui_glow::CallbackFn` into the current framebuffer, with the callback's
  viewport, and frees the context on the same thread before the mpv core is
  destroyed. mpv's update callback only calls `egui::Context::request_repaint`.
- **Events.** A dedicated thread blocks in `mpv_wait_event` and turns
  events into the existing `PlayerEvent`s. The IPC and embedded backends
  share one transport-independent event state machine, so progress, end
  and failure handling are identical.
- **Commands.** The UI's controls (pause, seek, volume, audio and subtitle
  track, stop) become typed commands in `cineo-player-mpv`. There is still
  no path for raw mpv commands (SECURITY.md §Player).
- **Fallback.** If libmpv cannot be loaded or is too old, playback uses the
  external mpv process from ADR-0004, which is kept as is. The reason is
  logged once.

## Consequences
- Video plays inside the Cineo window, under our own controls.
- `unsafe` code exists for the first time, limited to two modules of one
  crate. Mistakes there can crash the app. Every block is documented.
- libmpv is still needed at runtime. It is not statically linked. Bundling
  it (M7) must follow libmpv's license (shipped as a replaceable shared
  library). LEGAL.md changes from "not linked" to "loaded at runtime".
- Two backends exist until the external one can be removed. The shared
  event state machine keeps them consistent.
- Assumptions to verify during implementation: the GL context is current
  during eframe's `update` (needed to free the render context), and Wayland
  plus X11 rendering works through the render API. Both are INFERRED.
- Windows and macOS are untested, as before.
- Docs to change on acceptance: SECURITY.md (Unsafe code, Player), LEGAL.md,
  ARCHITECTURE.md (player), AGENTS.md (the ADR-0004 reference), DECISIONS.md
  (0004 → Superseded by 0014), COMPATIBILITY.md (Embedded playback),
  dev/DEPENDENCIES.md (`libloading`), GOALS.md and CHANGELOG.md.

## Rejected alternatives
- **Option 1:** it does not meet the goal.
- **Option 2:** it compiles LGPL Rust code into the MIT binary and makes
  libmpv a hard dependency at link time. The amount of FFI we need (about
  20 functions) does not justify that.
- **Option 3:** builds and CI would need libmpv development files, and the
  app could not start without libmpv. Option 4 avoids both at the cost of
  symbol lookup by name.
- **Option 5:** it does not work on Wayland.
