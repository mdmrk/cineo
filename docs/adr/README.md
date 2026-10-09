# Architecture decisions

The Architecture Decision Records (ADRs) in this folder, and what we took
from the reference projects. To add an ADR, copy [TEMPLATE.md](TEMPLATE.md).

An ADR is needed for:
- A new crate or a changed dependency direction.
- New core concepts or state flow.
- A new external dependency with an architectural impact (UI framework,
  database, player).
- Security policy changes.
- Dropping or adding protocol surface.

ADRs are never deleted. To change a decision, write a new ADR and mark the
old one *Superseded by NNNN*.

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-functional-core-effects-as-data.md) | Functional core with effects as data | Accepted |
| [0002](0002-workspace-and-crate-boundaries.md) | Minimal workspace; crates by IO boundary | Accepted |
| [0003](0003-lenient-wire-strict-domain.md) | Lenient wire parsing, strict domain types | Accepted |
| [0004](0004-mpv-external-process-player.md) | mpv as an external process over JSON IPC | Superseded by 0014 |
| [0005](0005-desktop-ui-tauri.md) | Desktop UI: Tauri 2 + web frontend | Superseded by 0011 |
| [0006](0006-network-safety-policy.md) | Network safety policy (SSRF, limits) | Accepted |
| [0007](0007-protocol-scope.md) | Protocol scope: HTTP transport only; no private Stremio APIs | Accepted |
| [0008](0008-agent-instruction-system.md) | AGENTS.md canonical; layered Claude rules and skills | Superseded by 0013 |
| [0009](0009-toolchain-and-msrv.md) | Pinned stable toolchain; MSRV equals the pin | Accepted |
| [0010](0010-stremio-parity-and-streaming-engine.md) | Stremio behavioral parity; own local streaming engine | Accepted (engine library: 0012; cache: superseded by 0015) |
| [0011](0011-desktop-ui-egui.md) | Desktop UI: egui (eframe, glow) with a custom theme | Accepted |
| [0012](0012-torrent-engine-librqbit.md) | Torrent engine: librqbit; BitTorrent traffic outside `cineo-net` | Accepted (cache and unpatched librqbit: superseded by 0015) |
| [0013](0013-local-agent-tooling.md) | Agent tool configuration is local; the repo keeps AGENTS.md and docs | Accepted |
| [0014](0014-embedded-libmpv-player.md) | Embedded playback with libmpv, loaded at runtime | Accepted |
| [0015](0015-no-torrent-cache-and-patched-librqbit.md) | No torrent cache; a vendored, patched librqbit | Accepted (vendoring: superseded by 0016; in-memory option: 0019) |
| [0016](0016-librqbit-from-a-fork.md) | librqbit from the owner's fork instead of a vendored copy | Accepted |
| [0017](0017-localization-with-fluent.md) | Localization: Fluent catalogs in the desktop shell; core reports typed messages | Accepted |
| [0018](0018-deep-links-single-instance-and-scheme-registration.md) | Deep links: typed routes in core, single-instance handover, opt-in scheme registration | Accepted |
| [0019](0019-torrent-data-in-memory.md) | Torrent data in memory, bounded by a sliding window | Accepted |

## What we took from the reference projects

### stremio-core (MIT): keep / modify / reject

| Concept | Verdict | Why |
|---------|---------|-----|
| Unidirectional state (Elm-like update → effects) | **Keep** | Testable without IO; the UI only renders state |
| Effects executed by an environment | **Modify** | Effects are a data enum interpreted by the shell, not boxed futures; easier to assert in tests (ADR-0001) |
| `Env` trait with static methods, generic over everything | **Modify** | Inject concrete IO clients into shells; the core does not need an environment at all |
| `ConditionalSend` / WASM-first futures | **Reject** | WASM is not a target; `Send` everywhere |
| `derive(Model)` macro, field-diff runtime | **Reject** | Premature; reconsider if UI change tracking becomes a bottleneck |
| `Ctx` god-object (profile, library, notifications, streams, …) | **Reject** | Separate state per feature; composed by the shell |
| Lenient serde (`DefaultOnError`, `VecSkipError`, …) | **Keep (concept)** | Real addon data needs it; we add warnings so the leniency is observable (ADR-0003) |
| Manifest filtering semantics | **Keep exactly** | Compatibility (ADDON_PROTOCOL.md) |
| Legacy / IPFS transports | **Reject** | ADR-0007 |
| Analytics module | **Reject** | No telemetry |
| Private account API + library sync | **Reject** | Not public; cloud sync not planned for now (owner decision 2026-10-06) |
| 25-step storage migrations | **Modify** | SQLite with versioned migrations from v1, each tested |
| Watched bitfield encoding | **Defer** | Revisit with series progress (M4) |
| MSRV CI job | **Modify** | We are an application: MSRV = pinned toolchain (ADR-0009) |

### stremio-web (GPL-2.0): ideas only, no code
- The core runs separately from the UI (in a worker) and the UI renders state
  snapshots. This confirms the UI boundary.
- Licensing: we do not reuse code, assets or styles.

### stremio-shell-ng (no license file: all rights reserved): ideas only
- mpv renders directly to a native surface, which avoids compositor overhead
  and keeps hardware decoding. This informs ADR-0004/0005.
- Anti-pattern rejected: the UI can send arbitrary `mpv-command`s.

### AnyPS5 (GPL-2.0): process ideas only
- Kept: the small `docs/user` and `docs/dev` split; a compatibility table that
  marks unknowns explicitly; a technical-debt log whose entries say what
  fails, why and what happens instead; fail loudly with no silent stubs;
  Conventional Commits; AI-assistance disclosure in PRs; least-privilege
  workflows with concurrency cancellation.
- Rejected: many bot workflows (labels, progress comments); tag-pinned
  actions; releases without checksums.
