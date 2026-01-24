//! Inter-process communication between the LSP proxy and TUI clients.
mod command_handler;
mod protocol;
mod server;

pub use command_handler::CommandHandler;
pub use protocol::{Command, CursorInfo, Message, Position, SOCKET_PATH};
pub use server::SocketServer;
