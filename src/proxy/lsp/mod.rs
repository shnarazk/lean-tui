//! LSP service implementations and utilities.
//!
//! Provides:
//! - Message interception and forwarding services
//! - Cursor position extraction from LSP messages
//! - Document content caching with tree-sitter parsing

mod cursor;
mod documents;
mod service;

pub use documents::DocumentCache;
pub use service::{DeferredService, InterceptService};
