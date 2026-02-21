//! Stream writer module - re-exports from ferrotick-agent.
//!
//! This module provides backward-compatible re-exports of the stream types
//! from the ferrotick-agent crate.

// Re-export stream types used by the CLI
pub use ferrotick_agent::stream::{NdjsonStreamWriter, StreamEventError};
