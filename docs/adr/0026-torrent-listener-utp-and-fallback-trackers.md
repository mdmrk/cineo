# 0026. Torrent engine: a listening port, uTP and fallback trackers

- Status: Accepted (amends 0012)
- Date: 2026-11-01

## Context
The goal is a torrent start as fast as Stremio's. Three limits in the
engine come from ADR-0012 and the way it is wired (VERIFIED from the code
of the librqbit fork at `57b643b6` and of `cineo-stream`):
- With `ConnectionOptions.proxy_url` set, librqbit connects to every peer
  through the SOCKS5 proxy and never tries uTP (`stream_connect.rs`). The
  same URL is also the proxy of its HTTP client, which is what filters
  HTTP trackers by host name.
- With `listen: None` there is no uTP socket and no incoming connection,
  yet `4240` is announced as our port.
- Trackers come only from the stream's `sources`. Torrentio sends none
  (VERIFIED 2026-10-06, ADDON_PROTOCOL.md), so the metadata and the first
  peers come from the DHT alone.

librqbit already filters peers by address itself: every outgoing peer
address (`live/mod.rs`, `blocked_outgoing`) and every incoming connection
(`session.rs`, `blocked_incoming`) is checked against
`SessionOptions.blocklist_url`, which may be a `file://` URL (VERIFIED from
the source). Peer addresses are IPs, never host names, so this check covers
them fully.

What Stremio's server does without `sources` is UNKNOWN. Public tracker
lists exist; ngosang/trackerslist `trackers_best.txt` was last updated
2026-10-31 (VERIFIED with curl, 2026-11-01).

## Options considered
1. Keep ADR-0012 as is: no uTP, no incoming peers, DHT only for Torrentio.
2. Drop the proxy entirely: peers and HTTP trackers direct; HTTP trackers
   by host name would lose the private-network filter.
3. Proxy only the HTTP client, and connect to peers directly over TCP and
   uTP, filtered by librqbit's blocklist; listen on a port; add a built-in
   list of public trackers.

## Decision
Option 3 (owner decisions 2026-11-01):
- **Fork change.** `SessionOptions.http_proxy_url` proxies librqbit's HTTP
  client only. `ConnectionOptions.proxy_url` keeps its upstream meaning.
- **Peers.** `cineo-stream` no longer gives librqbit a peer proxy. It writes
  its non-public address ranges (the same as the SOCKS5 policy, including
  IPv4-mapped, NAT64 and 6to4 forms) to `<cache>/blocklist.txt` and passes
  it as `blocklist_url`, unless the user allowed private networks. HTTP
  trackers still go through the SOCKS5 proxy.
- **Listening.** TCP and uTP on all interfaces (`[::]`, dual stack) on a
  port from Settings, 1024 to 65535, default 42713. If that port is taken,
  the engine listens on a random port instead and logs it. No UPnP; local
  service discovery stays off.
- **Fallback trackers.** Eight UDP trackers from ngosang/trackerslist
  `trackers_best.txt` (2026-10-31) are always added, through
  `SessionOptions.trackers`, to the stream's own.

## Consequences
- More reachable peers: uTP-only peers, and incoming peers over IPv6 or
  through a forwarded port. Torrentio streams get trackers besides the DHT.
- A port is open on every interface while a torrent engine runs. Incoming
  peers from non-public addresses are refused unless private networks are
  allowed. SECURITY.md lists it.
- Peer filtering moves from our SOCKS5 proxy to librqbit's blocklist, so
  its behavior is part of the fork review on every update.
- The fallback trackers are host names sent over UDP: like UDP trackers
  from addons, they are not checked against the private ranges
  (TECHNICAL_DEBT.md). The list ages and needs a review now and then.
- ADR-0012's "no incoming connections" is replaced by this decision; its
  other controls stay.

## Rejected alternatives
- Option 1: leaves the slowest part of the start (DHT-only discovery and
  TCP-only peers) in place.
- Option 2: HTTP trackers by host name, and their redirects, could reach
  private networks again; the proxy already handles that.
- UPnP: it opens a port on the router without asking each time; ADR-0012
  rejected it and nothing new changes that.
