//! NZ White meat rabbit planner: the simulation engine and solver (`engine`), and the transport-free
//! JSON API on top of it (`api`) that the HTTP server and the WASM build both expose.

pub mod api;
pub mod engine;
