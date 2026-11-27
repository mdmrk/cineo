# 0044. Remote control from the phone over the local network

- Status: Accepted
- Date: 2026-11-27
- Amends: 0006 (the phone sends requests to one paired private address)
  and 0014 (player commands can come from another device, still typed).

## Context
The owner asked on 2026-11-27 to control the desktop app from the phone
over the local network, "the way Steam lets you manage downloads from its
phone app". The first version covers what is already playing on the
computer: play and pause, seek, volume and mute, audio and subtitle
tracks, subtitle delay, speed and stop. Starting a new title from the
phone is left for later (owner, same day).

### What exists (VERIFIED: source, 2026-11-27)
- The desktop player takes only typed `PlayerCommand`s and publishes a
  `Status` snapshot (`cineo-player/src/embedded/mod.rs`). Media keys
  already reach it from outside the window: `media_keys::Remote` becomes
  commands on the UI thread (`app::remote_commands`).
- Speed and subtitle delay are not in `Status`; the desktop's `Controls`
  remember them (`player/mod.rs`).
- Sync pairing shows a 16-byte key as a 32-character code and a QR code,
  and Android scans QR codes with CameraX and ZXing (ADR-0043).
- `NetPolicy` blocks private addresses unless the user allows them
  (ADR-0006). Android 17 asks for `ACCESS_LOCAL_NETWORK` before LAN
  connections (`ui/LocalNetwork.kt`).
- hyper already serves the torrent stream on loopback (`cineo-stream`) and
  the sync server.

### Constraints
- Nothing on the LAN is trusted: anyone on the same Wi-Fi can reach an
  open port, read and replay traffic. There is no certificate a phone
  could check for a LAN address.
- No cloud relay: the owner wants LAN only, and the sync server must not
  learn what is playing.

## Options considered
1. Desktop listens on the LAN; the phone sends sealed requests with a key
   from a QR code. Pros: direct, no server, works offline. Cons: an open
   port, firewall prompts, the address can change.
2. Relay through the sync server. Pros: no open port. Cons: the server
   sees timing of every command; needs internet; the owner wants LAN.
3. TLS with a self-signed certificate pinned through the QR code. Pros:
   standard. Cons: rustls on the server side and a pinning verifier on the
   phone; more code for the same property as option 1.
4. mDNS discovery plus a PIN. Pros: no camera. Cons: a 6-digit PIN is
   guessable on the LAN without extra rate limiting and key agreement.

## Decision
Option 1.

- **Off by default.** Settings → Remote control on the desktop turns it
  on. Turning it on makes a random 16-byte key and starts the listener;
  turning it off forgets the key and stops it. "New code" replaces the
  key, which unpairs every phone.
- **Listener.** `cineo_net::remote::serve`, hyper HTTP/1 on
  `0.0.0.0:18765`, or a port the system picks when that one is taken.
  One route, `POST /v1/remote`; bodies of at most 4 KiB; at most 16
  connections; a request must arrive within 10 s.
- **Pairing.** The desktop shows `cineo:///remote/<code>?at=<ip>:<port>`
  as a QR code and as text. The code is the key in the sync code format.
  The phone scans it or takes the address and code typed in, and keeps
  them. The address must be private, loopback or link-local
  (`cineo_core::remote::is_lan`), so a link cannot point the phone at the
  internet.
- **Sealing.** HKDF-SHA256 of the key gives a ChaCha20-Poly1305 key.
  A request is `1 || nonce || sealed JSON` with associated data
  `cineo-remote-v1 request`; the answer is sealed the same way with
  `cineo-remote-v1 answer` and the request's nonce, so an old answer
  cannot be passed off as a new one. Whoever lacks the key can neither
  read nor forge either.
- **Replays.** A request carries its time; the desktop refuses one more
  than 2 minutes off its own clock, and one whose nonce it has already
  seen within that window (`cineo_core::remote::Replays`). Commands are
  absolute (pause or play, not toggle; volume 40, not up), so a request
  that slipped through twice does the same thing.
- **Commands.** `cineo_core::remote::Command` is a closed set with
  checked ranges. The desktop turns each into `PlayerCommand`s on the UI
  thread, as it does for media keys. Untrusted strings never reach mpv.
- **Status.** Each answer carries what is playing: title, pause, position,
  length, volume, mute, speed, subtitle delay and the audio and subtitle
  tracks. The phone asks every second while the remote screen is open.
- **Phone side.** `cineo_net::remote::RemoteClient` sends through an
  `AddonClient` whose policy allows private addresses, with short
  timeouts. It is only ever given the paired LAN address.
- **State.** The desktop key and the phone's paired computer are core
  state (`State::remote`), stored by `cineo-store` in tables that sync
  never reads, and restored at start.

## Consequences
- The desktop opens a TCP port on the LAN while the feature is on.
  Windows and macOS may ask to allow it through the firewall; a Linux
  firewall may need a rule. `docs/SECURITY.md` lists the port and its
  defenses.
- If the computer's address or port changes, the phone must scan again
  (`docs/TECHNICAL_DEBT.md`). Discovery with mDNS can come later.
- The computer's clock and the phone's must agree within 2 minutes.
- `cineo-net` now also holds a small server. It is the only listener on
  the LAN; the torrent stream stays on loopback.
- Docs: `ARCHITECTURE.md` (remote boundary), `SECURITY.md`,
  `COMPATIBILITY.md`, `TECHNICAL_DEBT.md`.

## Rejected alternatives
- Relay (2): needs the internet and gives the server timing data.
- TLS (3): same confidentiality and authenticity as the sealed bodies,
  with more code and a certificate to manage.
- PIN (4): too few bits to resist guessing on the LAN.
- A new crate for the listener: it shares the key code and protocol with
  the client and has no dependency of its own beyond hyper, which the
  workspace already uses (ADR-0002).
