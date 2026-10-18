//! The engine shared by both front ends: the iced GUI (`rust-ide`) and the
//! terminal UI (`rust-ide-tui`).

pub mod dap;
pub mod deps;
pub mod env;
pub mod filetype;
pub mod highlight;
pub mod lsp;
pub mod output;
pub mod platform;
pub mod runner;
pub mod search;
pub mod templates;
pub mod vim;
