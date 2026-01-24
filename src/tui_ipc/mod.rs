//! Inter-process communication between the LSP proxy and TUI clients.
//!
//! This module provides:
//! - `protocol`: Wire format types for messages between proxy and TUI
//! - `server`: Unix socket server that broadcasts to TUI clients  
//! - `command_handler`: Processes commands from TUI (e.g., navigation)

mod command_handler;
mod protocol;
mod server;

pub use command_handler::CommandHandler;
pub use protocol::{Command, CursorInfo, Message, Position, SOCKET_PATH};
pub use server::SocketServer;
