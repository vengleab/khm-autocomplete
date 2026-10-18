//! khtsf.dll: the Khmer autocomplete text service for the Windows Text Services Framework (TSF).
//!
//! It runs inside every app that uses the keyboard, so it is deliberately thin and defensive:
//! - no language data: it asks `khengine.exe` over a per-user named pipe (`khipc`), with hard deadlines;
//! - when the engine is missing or slow, keys pass through untouched and a circuit breaker stops asking for a while;
//! - no panic ever crosses a COM boundary (every entry point is wrapped in `catch_unwind`);
//! - no typed text is logged or stored.
//!
//! Keys are never remapped (no layout table here): the engine reads what was typed, so without the engine the DLL
//! only passes keys through.
//!
//! Everything except `logic` is Windows only.

pub mod logic;

#[cfg(windows)]
mod candwin;
#[cfg(windows)]
mod dll;
#[cfg(windows)]
mod edit;
#[cfg(windows)]
mod guids;
#[cfg(windows)]
mod link;
#[cfg(windows)]
mod reg;
#[cfg(windows)]
mod tip;
