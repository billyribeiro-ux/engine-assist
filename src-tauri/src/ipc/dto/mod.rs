//! Every IPC DTO beyond the shared §2 types (`docs/contracts/ipc.md` §4–§6), exported to TS by
//! ts-rs (ADR-004).

pub mod app;
pub mod doctor;
pub mod events;
pub mod gates;
pub mod launch;
pub mod records;
pub mod requests;
pub mod steps;
