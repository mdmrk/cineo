//! Pure domain core of Cineo.
//!
//! This crate owns the addon protocol model (manifests, resource requests,
//! response parsing) and, later, the application state model. It performs
//! **no IO**: no network, no filesystem, no clocks, no async runtime. Shells
//! and IO crates (`cineo-net`, the CLI, the desktop app) depend on it; it
//! depends on none of them. See `docs/ARCHITECTURE.md`.

pub mod addon;
pub mod diagnostics;
