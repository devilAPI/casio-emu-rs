//! Core of the fx-991ES PLUS C emulator.
//!
//! The CPU, memory map, LCD and key matrix are platform independent.  The
//! desktop frontend lives in `main.rs`; the WebAssembly frontend lives in the
//! `web` crate.

pub mod cpu;
pub mod keys;
pub mod machine;
pub mod model;
pub mod table;
