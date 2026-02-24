# Task: WebSocket Transport Module for Ferrotick

## Objective
Add WebSocket transport abstraction to ferrotick-core crate.

## Requirements
1. Add tokio-tungstenite and futures dependencies to Cargo.toml
2. Create transport module with websocket sub-module
3. Implement WebSocketTransport with connect, send, receive methods

## Step-by-Step Implementation

### Step 1: Add dependencies
**File:** `crates/ferrotick-core/Cargo.toml`
**Action:** modify
**Location:** After line 19 (after `urlencoding.workspace = true`), before `[dev-dependencies]`
**What to do:** Add two new dependencies
**Code:**
```toml
tokio-tungstenite = "0.26"
futures = "0.3"
```
**Notes:** tokio-tungstenite is the standard async WebSocket library for Rust

### Step 2: Create transport module directory
**File:** `crates/ferrotick-core/src/transport/mod.rs`
**Action:** create
**Location:** New file
**What to do:** Create module file that exports the websocket module
**Code:**
```rust
//! Transport layer for provider adapters.

pub mod websocket;

pub use websocket::{WebSocketTransport, WebSocketError};
```
**Notes:** This follows the same pattern as other modules in src/lib.rs

### Step 3: Create WebSocket transport implementation
**File:** `crates/ferrotick-core/src/transport/websocket.rs`
**Action:** create
**Location:** New file
**What to do:** Implement the WebSocketTransport struct with connect, send, receive methods
**Code:**
```rust
//! WebSocket transport implementation.

use std::fmt::{Display, Formatter};
use tokio_tungstenite::{
    tungstenite::protocol::Message,
    MaybeTlsStream,
    WebSocketStream,
};
use tokio::net::TcpStream;

/// WebSocket transport error.
#[derive(Debug, thiserror::Error)]
pub enum WebSocketError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("send failed: {0}")]
    SendFailed(String),

    #[error("receive failed: {0}")]
    ReceiveFailed(String),

    #[error("already connected")]
    AlreadyConnected,

    #[error("not connected")]
    NotConnected,

    #[error("closed: {0}")]
    Closed(String),
}

/// WebSocket transport client.
pub struct WebSocketTransport {
    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

impl WebSocketTransport {
    /// Create a new disconnected transport.
    pub fn new() -> Self {
        Self { stream: None }
    }

    /// Connect to a WebSocket URL.
    pub async fn connect(url: &str) -> Result<Self, WebSocketError> {
        let (stream, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| WebSocketError::ConnectionFailed(e.to_string()))?;

        Ok(Self { stream: Some(stream) })
    }

    /// Send a text message.
    pub async fn send(&mut self, msg: &str) -> Result<(), WebSocketError> {
        let stream = self.stream
            .as_mut()
            .ok_or(WebSocketError::NotConnected)?;

        stream
            .send(Message::Text(msg.to_string()))
            .await
            .map_err(|e| WebSocketError::SendFailed(e.to_string()))?;

        Ok(())
    }

    /// Receive the next message.
    pub async fn receive(&mut self) -> Result<Option<String>, WebSocketError> {
        let stream = self.stream
            .as_mut()
            .ok_or(WebSocketError::NotConnected)?;

        match stream.next().await {
            Some(Ok(Message::Text(text))) => Ok(Some(text)),
            Some(Ok(Message::Close(_))) => {
                self.stream = None;
                Ok(None)
            }
            Some(Ok(_)) => Ok(None),
            Some(Err(e)) => Err(WebSocketError::ReceiveFailed(e.to_string())),
            None => {
                self.stream = None;
                Ok(None)
            }
        }
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }
}

impl Default for WebSocketTransport {
    fn default() -> Self {
        Self::new()
    }
}
```
**Notes:** Uses tokio-tungstenite standard patterns with async methods

### Step 4: Export transport module from lib.rs
**File:** `crates/ferrotick-core/src/lib.rs`
**Action:** modify
**Location:** Find the module declarations section (around line 68 after `pub mod throttling;`), add new line
**What to do:** Add transport module declaration and re-exports
**Code:**
Add after line containing `pub mod throttling;`:
```rust
pub mod transport;
```

Then in the re-exports section (around line 107 after `pub use throttling::ThrottlingQueue;`), add:
```rust
pub use transport::{WebSocketTransport, WebSocketError};
```
**Notes:** This makes the transport types available at crate root for convenience

## Existing Patterns to Follow
- Error handling with `thiserror::Error` (from `src/error.rs`)
- Module structure with `pub mod` and `pub use` (from `src/lib.rs`)
- Async methods with `pub async fn` (from `src/http_client.rs`)

## Edge Cases and Error Handling
1. **Calling send/receive without connecting** → Returns `WebSocketError::NotConnected`
2. **Connection fails** → Returns `WebSocketError::ConnectionFailed` with details
3. **Server closes connection** → Returns `Ok(None)` and marks as disconnected
4. **Non-text messages** → Silently ignored, returns `Ok(None)`

## Dependencies and Imports
- `tokio-tungstenite = "0.26"` for WebSocket client
- `futures = "0.3"` for stream handling (via tokio-tungstenite)
- `thiserror` (already in workspace) for error derive

## Acceptance Criteria
- [ ] `cargo build -p ferrotick-core` compiles without errors
- [ ] `cargo build` (workspace) compiles without errors
- [ ] Module is properly exported: `use ferrotick_core::WebSocketTransport;` works

## Out of Scope
- No streaming adapters
- No CLI integration
- No provider implementations
- No reconnection logic
- No message protocol parsing

---STOP NOW. Write PLAN.md to the project root. Do not explore further.---
