//! # Streaming Consumer Example
//!
//! This example demonstrates how to consume NDJSON streaming output
//! from Ferrotick, which is useful for AI agents and real-time
//! data processing pipelines.
//!
//! ## Usage
//!
//! ```bash
//! # In one terminal, run ferrotick with streaming enabled
//! ferrotick quote AAPL MSFT GOOGL --stream > /tmp/quotes.ndjson
//!
//! # In another terminal, run this consumer
//! cargo run --example streaming_consumer < /tmp/quotes.ndjson
//! ```
//!
//! ## NDJSON Stream Events
//!
//! The stream emits the following event types:
//! - `start` - Operation initiated
//! - `progress` - Status update during operation
//! - `chunk` - Data batch delivered
//! - `end` - Operation completed
//! - `error` - Error occurred

use std::io::{self, BufRead};
use serde::Deserialize;

/// Stream event types from Ferrotick
#[derive(Debug, Deserialize)]
#[serde(tag = "event")]
enum StreamEvent {
    #[serde(rename = "start")]
    Start {
        request_id: String,
        operation: String,
        timestamp: String,
    },
    #[serde(rename = "progress")]
    Progress {
        message: String,
        source: Option<String>,
    },
    #[serde(rename = "chunk")]
    Chunk {
        data: serde_json::Value,
        sequence: Option<u64>,
    },
    #[serde(rename = "end")]
    End {
        request_id: String,
        latency_ms: u64,
        total_chunks: Option<u64>,
    },
    #[serde(rename = "error")]
    Error {
        code: String,
        message: String,
        source: Option<String>,
    },
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut chunks_received = 0;
    let mut start_time: Option<String> = None;

    println!("🔄 Starting stream consumer...");
    println!("{}", "=".repeat(50));

    for line in stdin.lock().lines() {
        let line = line?;
        
        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Parse the event
        match serde_json::from_str::<StreamEvent>(&line) {
            Ok(event) => {
                match event {
                    StreamEvent::Start { request_id, operation, timestamp } => {
                        start_time = Some(timestamp.clone());
                        println!("\n🚀 START: {}", operation);
                        println!("   Request ID: {}", request_id);
                        println!("   Timestamp: {}", timestamp);
                    }
                    StreamEvent::Progress { message, source } => {
                        print!("⏳ PROGRESS: {}", message);
                        if let Some(src) = source {
                            print!(" (from {})", src);
                        }
                        println!();
                    }
                    StreamEvent::Chunk { data, sequence } => {
                        chunks_received += 1;
                        print!("\n📦 CHUNK");
                        if let Some(seq) = sequence {
                            print!(" #{}", seq);
                        }
                        println!(" received:");
                        
                        // Pretty-print the data
                        if let Ok(pretty) = serde_json::to_string_pretty(&data) {
                            for line in pretty.lines().take(10) {
                                println!("   {}", line);
                            }
                            if pretty.lines().count() > 10 {
                                println!("   ... (truncated)");
                            }
                        }
                    }
                    StreamEvent::End { request_id, latency_ms, total_chunks } => {
                        println!("\n✅ END");
                        println!("   Request ID: {}", request_id);
                        println!("   Latency: {}ms", latency_ms);
                        if let Some(total) = total_chunks {
                            println!("   Total chunks: {}", total);
                        }
                        println!("   Chunks received by consumer: {}", chunks_received);
                    }
                    StreamEvent::Error { code, message, source } => {
                        println!("\n❌ ERROR [{}]: {}", code, message);
                        if let Some(src) = source {
                            println!("   Source: {}", src);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("⚠️  Failed to parse line: {}", e);
                eprintln!("   Line: {}", line.chars().take(100).collect::<String>());
            }
        }
    }

    println!("\n{}", "=".repeat(50));
    println!("🏁 Stream finished. Total chunks: {}", chunks_received);

    Ok(())
}
