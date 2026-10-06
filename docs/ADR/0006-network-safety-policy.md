# 0006. Network safety policy

- Status: Accepted
- Date: 2026-10-06

## Context
Addons are untrusted servers whose responses contain more URLs (images, streams, subtitles, redirects). A malicious addon could make the client attack the user's LAN (router admin, NAS, cloud metadata) — client-side SSRF. OWASP recommends validating resolved IPs and connecting only to validated addresses, re-validating redirects, and blocking special-purpose ranges (VERIFIED: OWASP SSRF cheat sheet). reqwest supports a custom DNS resolver and redirect policy (VERIFIED: compiled against reqwest 0.13.5 in a prototype).

## Options considered
1. No restrictions (typical media client).
2. Pre-flight hostname check only — bypassable by DNS rebinding.
3. Resolver-level filtering + literal-IP checks + redirect re-validation + size/time limits, with explicit opt-in for private networks.

## Decision
Option 3, implemented in `cineo-net` (`NetPolicy`, `AddonClient`), defaults in SECURITY.md. Private-network access is off by default and enabled only by explicit user action (CLI flag now, per-addon trust later). System proxies are not used for addon traffic. Full addon URLs never appear in logs or errors.

## Consequences
- Self-hosted LAN addons need an explicit opt-in (UX cost, documented).
- Tests must opt in for loopback mock servers; one test pins the default block.
- The same policy must later apply to images, subtitles and stream URLs handed to mpv (M3/M6).

## Rejected alternatives
Option 1 exposes the LAN. Option 2 is bypassable.
