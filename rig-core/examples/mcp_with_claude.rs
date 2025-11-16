//! Example demonstrating how to use Model Context Protocol (MCP) with Claude (Anthropic).
//!
//! This example shows how to:
//! 1. Set up an MCP server with tools
//! 2. Connect a Claude agent to the MCP server
//! 3. Use MCP tools in agent workflows
//!
//! # Running the example
//!
//! You'll need to set your Anthropic API key:
//! ```bash
//! export ANTHROPIC_API_KEY="your-api-key"
//! cargo run --example mcp_with_claude --features rmcp
//! ```

use rig::{
    completion::Prompt,
    providers::anthropic::{self, CLAUDE_3_5_SONNET},
};
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};

/// Request structure for calculator operations
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CalculatorRequest {
    pub operation: String,
    pub a: f64,
    pub b: f64,
}

/// Request structure for getting current time
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TimeRequest {
    pub timezone: Option<String>,
}

/// MCP Server that provides calculator and time tools
#[derive(Clone)]
pub struct AgentToolServer {
    counter: Arc<Mutex<i32>>,
    tool_router: ToolRouter<AgentToolServer>,
}

impl Default for AgentToolServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl AgentToolServer {
    pub fn new() -> Self {
        Self {
            counter: Arc::new(Mutex::new(0)),
            tool_router: Self::tool_router(),
        }
    }

    /// Calculator tool that performs basic arithmetic operations
    #[tool(description = "Perform basic arithmetic operations (add, subtract, multiply, divide)")]
    fn calculate(
        &self,
        Parameters(CalculatorRequest { operation, a, b }): Parameters<CalculatorRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = match operation.as_str() {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(ErrorData::invalid_params(
                        "division_by_zero",
                        Some(json!({"message": "Cannot divide by zero"})),
                    ));
                }
                a / b
            }
            _ => {
                return Err(ErrorData::invalid_params(
                    "unknown_operation",
                    Some(json!({"message": format!("Unknown operation: {}", operation)})),
                ))
            }
        };

        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    /// Get the current timestamp
    #[tool(description = "Get the current time and date")]
    fn get_current_time(
        &self,
        Parameters(TimeRequest { timezone: _ }): Parameters<TimeRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let now = chrono::Utc::now();
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Current UTC time: {}",
            now.to_rfc3339()
        ))]))
    }

    /// Increment a counter (demonstration of stateful tool)
    #[tool(description = "Increment an internal counter and return the new value")]
    async fn increment_counter(&self) -> Result<CallToolResult, ErrorData> {
        let mut counter = self.counter.lock().await;
        *counter += 1;
        Ok(CallToolResult::success(vec![Content::text(
            counter.to_string(),
        )]))
    }

    /// Get the current counter value
    #[tool(description = "Get the current counter value")]
    async fn get_counter(&self) -> Result<CallToolResult, ErrorData> {
        let counter = self.counter.lock().await;
        Ok(CallToolResult::success(vec![Content::text(
            counter.to_string(),
        )]))
    }
}

#[tool_handler]
impl ServerHandler for AgentToolServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This MCP server provides tools for Claude Code integration including:\n\
                - Calculator for basic arithmetic operations\n\
                - Current time retrieval\n\
                - Counter manipulation (stateful operations)"
                    .to_string(),
            ),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        Ok(self.get_info())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Start MCP server on localhost:3000
    let service = TowerToHyperService::new(StreamableHttpService::new(
        || Ok(AgentToolServer::new()),
        LocalSessionManager::default().into(),
        Default::default(),
    ));

    let listener = tokio::net::TcpListener::bind("localhost:3000").await?;
    println!("MCP Server started on http://localhost:3000");

    // Spawn server in background
    tokio::spawn({
        let service = service.clone();
        async move {
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        println!("Received Ctrl+C, shutting down");
                        break;
                    }
                    accept = listener.accept() => {
                        match accept {
                            Ok((stream, _addr)) => {
                                let io = TokioIo::new(stream);
                                let service = service.clone();

                                tokio::spawn(async move {
                                    if let Err(e) = Builder::new(TokioExecutor::default())
                                        .serve_connection(io, service)
                                        .await
                                    {
                                        eprintln!("Connection error: {e:?}");
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("Accept error: {e:?}");
                            }
                        }
                    }
                }
            }
        }
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Connect to MCP server
    let transport =
        rmcp::transport::StreamableHttpClientTransport::from_uri("http://localhost:3000");

    let client_info = ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "rig-claude-mcp-client".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        },
    };

    let client = client_info
        .serve(transport)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to MCP server: {:?}", e))?;

    println!("Connected to MCP server");
    let server_info = client.peer_info();
    println!("Server info: {:#?}", server_info);

    // List available tools
    let tools: Vec<Tool> = client.list_tools(Default::default()).await?.tools;
    println!("\nAvailable MCP tools:");
    for tool in &tools {
        println!("  - {}: {}", tool.name, tool.description.as_ref().unwrap_or(&"".into()));
    }

    // Create Claude agent with MCP tools
    let anthropic_client =
        anthropic::Client::new(&env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set"));

    let agent = anthropic_client
        .agent(CLAUDE_3_5_SONNET)
        .preamble(
            "You are a helpful assistant with access to tools via Model Context Protocol (MCP). \
            Use the available tools to answer user questions accurately.",
        )
        .rmcp_tools(tools, client.peer().to_owned())
        .build();

    println!("\n=== Claude + MCP Demo ===\n");

    // Example 1: Calculator usage
    println!("Example 1: Using calculator");
    let response = agent
        .prompt("What is 123 multiplied by 456? Please use the calculator tool.")
        .multi_turn(3)
        .await?;
    println!("Claude: {}\n", response);

    // Example 2: Time query
    println!("Example 2: Getting current time");
    let response = agent
        .prompt("What is the current time?")
        .multi_turn(3)
        .await?;
    println!("Claude: {}\n", response);

    // Example 3: Counter operations
    println!("Example 3: Counter operations");
    let response = agent
        .prompt("Please increment the counter twice and then tell me the current value.")
        .multi_turn(5)
        .await?;
    println!("Claude: {}\n", response);

    // Example 4: Complex calculation
    println!("Example 4: Complex calculation");
    let response = agent
        .prompt(
            "Calculate (25 + 75) divided by 10, then multiply that result by 3. \
            Show your work using the calculator tool.",
        )
        .multi_turn(5)
        .await?;
    println!("Claude: {}\n", response);

    Ok(())
}
