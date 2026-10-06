//! Pure domain core of Cineo.
//!
//! This crate will own the addon protocol model (manifests, resource
//! requests, response parsing) and the application state model. It performs
//! **no IO**: no network, no filesystem, no clocks, no async runtime. Shells
//! and IO crates depend on it; it depends on none of them.
//!
//! Status: placeholder. Implementation starts at Milestone 1
//! (`docs/ROADMAP.md`); the design is in `docs/ARCHITECTURE.md`.
