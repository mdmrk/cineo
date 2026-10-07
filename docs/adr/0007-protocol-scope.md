# 0007. Protocol scope: HTTP transport only; no private Stremio APIs

- Status: Accepted
- Date: 2026-10-06

## Context
The SDK docs list HTTP, legacy (`/stremio/v1`, JSON-RPC for v1/v2 addons) and IPFS transports (VERIFIED-DOC). The reference client implements HTTP and legacy; IPFS is UNKNOWN in current core. Stremio also uses a private account API (`api.strem.io`) and a separately distributed streaming server for torrents/archives; neither is a public protocol (VERIFIED: not in the open repos; documentation absent).

## Decision
Implement the HTTP transport only. Reject legacy and IPFS. Do not call Stremio's private APIs or depend on its streaming server. Stream sources that need a streaming engine are parsed and shown as unsupported.

## Consequences
- Old v1/v2 addons do not work (they are rare; INFERRED).
- No account import from Stremio. Sync, if ever, is our own design.
- Torrent support is a separate research decision (GOALS.md).

## Rejected alternatives
Legacy transport: extra protocol for a shrinking set of addons. IPFS: niche, heavy dependency. Private APIs: undocumented, unstable, and legally unclear (LEGAL.md).
