//! Command handlers, one module per group (`docs/contracts/ipc.md` §4). Each handler is a plain
//! function of the app state and the validated request, so tests call it directly.

pub mod app;
pub mod assembler;
pub mod chat;
pub mod decisions;
pub mod desktop;
pub mod doctor;
pub mod files;
pub mod first_run;
pub mod gates;
pub mod launch;
pub mod packets;
pub mod projects;
pub mod settings;
pub mod signing;
pub mod steps;
pub mod terminal;
