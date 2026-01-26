//! Inter-process communication between the LSP proxy and TUI clients.
//!
//! This module provides:
//! - `protocol`: Shared types for serialization over Unix socket
//! - `proxy_endpoint`: Proxy-side server and command handler
//! - `tui_endpoint`: TUI-side client connection

mod protocol;
mod proxy_endpoint;
mod tui_endpoint;

// Re-export protocol types (shared between both processes)
pub use protocol::{socket_path, Command, CursorInfo, GoalResult, Message, Position, TemporalSlot};
// Re-export proxy-side types
pub use proxy_endpoint::{CommandHandler, SocketServer};
// Re-export TUI-side types
pub use tui_endpoint::spawn_socket_handler;
