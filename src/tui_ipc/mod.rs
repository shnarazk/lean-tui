//! Inter-process communication between the LSP proxy and TUI clients.
//!
//! This module provides:
//! - `protocol`: Shared types for serialization over Unix socket
//! - `proxy_endpoint`: Proxy-side server and command handler
//! - `tui_endpoint`: TUI-side client connection

mod protocol;
mod proxy_endpoint;
mod tui_endpoint;

pub use protocol::{
    socket_path, CaseSplitInfo, Command, CursorInfo, DefinitionInfo, GoalResult, Message, Position,
    ProofStep, ProofStepSource, TemporalSlot,
};
pub use proxy_endpoint::{CommandHandler, SocketServer};
pub use tui_endpoint::spawn_socket_handler;
