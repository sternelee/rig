//! Helper utilities for Model Context Protocol (MCP) integration.
//!
//! This module provides documentation and type re-exports for MCP integration.
//! For working examples, see:
//! - `examples/mcp_with_claude.rs`
//! - `examples/mcp_with_gemini.rs`
//! - `examples/rmcp.rs`
//!
//! # Quick Start
//!
//! To use MCP with Rig, you need to:
//! 1. Enable the `rmcp` feature in rig-core
//! 2. Add rmcp with appropriate transport features to your dependencies
//! 3. Connect to an MCP server
//! 4. List available tools
//! 5. Add the tools to your agent using `.rmcp_tools()`
//!
//! # Example
//!
//! ```no_run
//! use rig::{client::CompletionClient, completion::Prompt, providers::anthropic};
//! use rmcp::ServiceExt;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Connect to MCP server
//! let transport = rmcp::transport::StreamableHttpClientTransport::from_uri("http://localhost:3000");
//! let client_info = rmcp::model::ClientInfo {
//!     protocol_version: Default::default(),
//!     capabilities: Default::default(),
//!     client_info: rmcp::model::Implementation {
//!         name: "my-agent".to_string(),
//!         version: "1.0.0".to_string(),
//!         ..Default::default()
//!     },
//! };
//!
//! let client = client_info.serve(transport).await?;
//! let tools = client.list_tools(Default::default()).await?.tools;
//!
//! // Create agent with MCP tools
//! let anthropic_client = anthropic::Client::from_env();
//! let agent = anthropic_client
//!     .agent(anthropic::CLAUDE_3_5_SONNET)
//!     .preamble("You are a helpful assistant with MCP tools.")
//!     .rmcp_tools(tools, client.peer().to_owned())
//!     .build();
//!
//! let response = agent.prompt("Use the tools to help me").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Required Dependencies
//!
//! Add to your `Cargo.toml`:
//! ```toml
//! [dependencies]
//! rig-core = { version = "0.24", features = ["rmcp"] }
//! rmcp = { version = "0.8", features = [
//!     "client",
//!     "transport-streamable-http-client-reqwest",
//! ] }
//! ```

#![cfg(feature = "rmcp")]
#![cfg_attr(docsrs, doc(cfg(feature = "rmcp")))]

// Re-export commonly used rmcp types for convenience
pub use rmcp::model::{ClientCapabilities, ClientInfo, Implementation, ProtocolVersion, Tool};
pub use rmcp::ServiceExt;

#[cfg(test)]
mod tests {
    #[test]
    fn test_mcp_module_exists() {
        // This test just ensures the module compiles with the rmcp feature
        assert!(true);
    }
}
