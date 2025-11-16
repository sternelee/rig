//! Helper utilities for Model Context Protocol (MCP) integration.
//!
//! This module provides convenience functions and builders for setting up
//! MCP servers and clients with Rig agents.

#![cfg(feature = "rmcp")]
#![cfg_attr(docsrs, doc(cfg(feature = "rmcp")))]

use rmcp::model::{ClientInfo, Implementation, ProtocolVersion, ClientCapabilities};
use rmcp::ServiceExt;
use std::error::Error as StdError;

/// Builder for creating MCP client connections
///
/// # Example
/// ```no_run
/// use rig::mcp::McpClientBuilder;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = McpClientBuilder::new("http://localhost:3000")
///     .client_name("my-agent")
///     .client_version("1.0.0")
///     .connect()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct McpClientBuilder {
    uri: String,
    client_name: String,
    client_version: String,
}

impl McpClientBuilder {
    /// Create a new MCP client builder with the server URI
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            client_name: "rig-client".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Set the client name
    pub fn client_name(mut self, name: impl Into<String>) -> Self {
        self.client_name = name.into();
        self
    }

    /// Set the client version
    pub fn client_version(mut self, version: impl Into<String>) -> Self {
        self.client_version = version.into();
        self
    }

    /// Connect to the MCP server
    pub async fn connect(self) -> Result<rmcp::service::ServerSink, Box<dyn StdError + Send + Sync>> {
        let transport =
            rmcp::transport::StreamableHttpClientTransport::from_uri(&self.uri);

        let client_info = ClientInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: self.client_name,
                version: self.client_version,
                ..Default::default()
            },
        };

        let client = client_info
            .serve(transport)
            .await
            .map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)?;

        Ok(client.peer().to_owned())
    }
}

/// Quick connect to an MCP server at the given URI
///
/// # Example
/// ```no_run
/// use rig::mcp;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = mcp::connect("http://localhost:3000").await?;
/// # Ok(())
/// # }
/// ```
pub async fn connect(uri: impl Into<String>) -> Result<rmcp::service::ServerSink, Box<dyn StdError + Send + Sync>> {
    McpClientBuilder::new(uri).connect().await
}

/// List available tools from an MCP server
///
/// # Example
/// ```no_run
/// use rig::mcp;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = mcp::connect("http://localhost:3000").await?;
/// let tools = mcp::list_tools(&client).await?;
/// for tool in &tools {
///     println!("{}: {}", tool.name, tool.description.as_ref().unwrap_or(&"".into()));
/// }
/// # Ok(())
/// # }
/// ```
pub async fn list_tools(
    client: &rmcp::service::ServerSink,
) -> Result<Vec<rmcp::model::Tool>, Box<dyn StdError + Send + Sync>> {
    let result = client
        .list_tools(Default::default())
        .await
        .map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)?;
    Ok(result.tools)
}

/// Helper to create an MCP-enabled agent
///
/// This is a convenience wrapper that connects to an MCP server and
/// configures an agent with the available tools.
///
/// # Example
/// ```no_run
/// use rig::mcp;
/// use rig::providers::anthropic;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let anthropic_client = anthropic::Client::from_env();
/// let agent = mcp::agent_with_tools(
///     anthropic_client.agent(anthropic::CLAUDE_3_5_SONNET),
///     "http://localhost:3000"
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn agent_with_tools<M>(
    agent_builder: crate::agent::AgentBuilder<M>,
    mcp_server_uri: impl Into<String>,
) -> Result<crate::agent::AgentBuilderSimple<M>, Box<dyn StdError + Send + Sync>>
where
    M: crate::completion::CompletionModel,
{
    let uri = mcp_server_uri.into();
    let transport = rmcp::transport::StreamableHttpClientTransport::from_uri(&uri);

    let client_info = ClientInfo {
        protocol_version: ProtocolVersion::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "rig-client".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        },
    };

    let client = client_info
        .serve(transport)
        .await
        .map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)?;

    let tools = client.list_tools(Default::default()).await
        .map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)?
        .tools;

    Ok(agent_builder.rmcp_tools(tools, client.peer().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_client_builder() {
        let builder = McpClientBuilder::new("http://localhost:3000")
            .client_name("test-client")
            .client_version("0.1.0");

        assert_eq!(builder.uri, "http://localhost:3000");
        assert_eq!(builder.client_name, "test-client");
        assert_eq!(builder.client_version, "0.1.0");
    }
}
