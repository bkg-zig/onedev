//! Repositories whose row types live in this crate.
//!
//! `Session` is an auth type, so the repository stays here. Putting it in
//! `onedev-db` would make that crate depend on `onedev-auth`.

pub mod session;

pub use session::*;
