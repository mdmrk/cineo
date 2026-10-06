//! Network IO for Cineo.
//!
//! Everything fetched here is untrusted. The [`AddonClient`] enforces the
//! network policy described in `docs/SECURITY.md`: scheme allowlist,
//! private-network blocking (checked on resolved addresses, so DNS rebinding
//! cannot bypass it), redirect re-validation, timeouts and body size caps.

mod addon_client;
mod policy;

pub use addon_client::{AddonClient, FetchError};
pub use policy::{BlockReason, NetPolicy, is_public_ip};
