//! LSP proxy that sits between editor and lake serve.
//!
//! Architecture:
//! ```text
//! Editor ←→ [Intercept] ←→ lake serve
//!              ↓
//!         Broadcaster → TUI clients
//! ```

mod cursor_extractor;
mod forward;
mod intercept;
mod proxy;

pub use proxy::run;
