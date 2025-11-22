//! Example demonstrating how to use Model Context Protocol (MCP) with Gemini.
//!
//! This example shows how to:
//! 1. Set up an MCP server with tools
//! 2. Connect a Gemini agent to the MCP server
//! 3. Use MCP tools in agent workflows
//!
//! # Running the example
//!
//! You'll need to set your Google AI API key:
//! ```bash
//! export GEMINI_API_KEY="your-api-key"
//! cargo run --example mcp_with_gemini --features rmcp
//! ```

use rig::{
    client::CompletionClient,
    completion::Prompt,
    providers::gemini,
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

/// Request for text analysis
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TextAnalysisRequest {
    pub text: String,
}

/// Request for unit conversion
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UnitConversionRequest {
    pub value: f64,
    pub from_unit: String,
    pub to_unit: String,
}

/// Request for data storage
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DataStoreRequest {
    pub key: String,
    pub value: String,
}

/// Request for data retrieval
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DataRetrieveRequest {
    pub key: String,
}

/// MCP Server that provides various utility tools
#[derive(Clone)]
pub struct GeminiToolServer {
    data_store: Arc<Mutex<std::collections::HashMap<String, String>>>,
    tool_router: ToolRouter<GeminiToolServer>,
}

impl Default for GeminiToolServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl GeminiToolServer {
    pub fn new() -> Self {
        Self {
            data_store: Arc::new(Mutex::new(std::collections::HashMap::new())),
            tool_router: Self::tool_router(),
        }
    }

    /// Analyze text and return word count and character count
    #[tool(description = "Analyze text to get word count, character count, and other statistics")]
    fn analyze_text(
        &self,
        Parameters(TextAnalysisRequest { text }): Parameters<TextAnalysisRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let word_count = text.split_whitespace().count();
        let char_count = text.chars().count();
        let char_count_no_spaces = text.chars().filter(|c| !c.is_whitespace()).count();
        let line_count = text.lines().count();

        let analysis = json!({
            "word_count": word_count,
            "character_count": char_count,
            "character_count_no_spaces": char_count_no_spaces,
            "line_count": line_count,
        });

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Text Analysis:\n{}",
            serde_json::to_string_pretty(&analysis).unwrap()
        ))]))
    }

    /// Convert between common units
    #[tool(description = "Convert between units (supports: km-miles, kg-pounds, celsius-fahrenheit)")]
    fn convert_units(
        &self,
        Parameters(UnitConversionRequest {
            value,
            from_unit,
            to_unit,
        }): Parameters<UnitConversionRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = match (from_unit.as_str(), to_unit.as_str()) {
            ("km", "miles") | ("kilometers", "miles") => value * 0.621371,
            ("miles", "km") | ("miles", "kilometers") => value / 0.621371,
            ("kg", "pounds") | ("kilograms", "pounds") => value * 2.20462,
            ("pounds", "kg") | ("pounds", "kilograms") => value / 2.20462,
            ("celsius", "fahrenheit") => (value * 9.0 / 5.0) + 32.0,
            ("fahrenheit", "celsius") => (value - 32.0) * 5.0 / 9.0,
            _ => {
                return Err(ErrorData::invalid_params(
                    "unsupported_conversion",
                    Some(json!({
                        "message": format!("Unsupported conversion: {} to {}", from_unit, to_unit)
                    })),
                ))
            }
        };

        Ok(CallToolResult::success(vec![Content::text(format!(
            "{} {} = {} {}",
            value, from_unit, result, to_unit
        ))]))
    }

    /// Store a key-value pair
    #[tool(description = "Store a value with a key for later retrieval")]
    async fn store_data(
        &self,
        Parameters(DataStoreRequest { key, value }): Parameters<DataStoreRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut store = self.data_store.lock().await;
        store.insert(key.clone(), value);
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Stored data with key: {}",
            key
        ))]))
    }

    /// Retrieve a stored value by key
    #[tool(description = "Retrieve a previously stored value by its key")]
    async fn retrieve_data(
        &self,
        Parameters(DataRetrieveRequest { key }): Parameters<DataRetrieveRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let store = self.data_store.lock().await;
        if let Some(value) = store.get(&key) {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Value for '{}': {}",
                key, value
            ))]))
        } else {
            Err(ErrorData::invalid_params(
                "key_not_found",
                Some(json!({
                    "message": format!("No data found for key: {}", key)
                })),
            ))
        }
    }

    /// List all stored keys
    #[tool(description = "List all stored data keys")]
    async fn list_keys(&self) -> Result<CallToolResult, ErrorData> {
        let store = self.data_store.lock().await;
        let keys: Vec<String> = store.keys().cloned().collect();
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Stored keys: {:?}",
            keys
        ))]))
    }
}

#[tool_handler]
impl ServerHandler for GeminiToolServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This MCP server provides tools for Gemini CLI integration including:\n\
                - Text analysis (word count, character count, etc.)\n\
                - Unit conversion (distance, weight, temperature)\n\
                - Simple key-value data storage"
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

    // Start MCP server on localhost:3001
    let service = TowerToHyperService::new(StreamableHttpService::new(
        || Ok(GeminiToolServer::new()),
        LocalSessionManager::default().into(),
        Default::default(),
    ));

    let listener = tokio::net::TcpListener::bind("localhost:3001").await?;
    println!("MCP Server started on http://localhost:3001");

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
        rmcp::transport::StreamableHttpClientTransport::from_uri("http://localhost:3001");

    let client_info = ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "rig-gemini-mcp-client".to_string(),
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
        println!(
            "  - {}: {}",
            tool.name,
            tool.description.as_ref().unwrap_or(&"".into())
        );
    }

    // Create Gemini agent with MCP tools
    let gemini_client =
        gemini::Client::new(&env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY not set"));

    let agent = gemini_client
        .agent(gemini::completion::GEMINI_2_0_FLASH)
        .preamble(
            "You are a helpful assistant with access to tools via Model Context Protocol (MCP). \
            Use the available tools to answer user questions accurately and efficiently.",
        )
        .rmcp_tools(tools, client.peer().to_owned())
        .build();

    println!("\n=== Gemini + MCP Demo ===\n");

    // Example 1: Text analysis
    println!("Example 1: Text analysis");
    let response = agent
        .prompt(
            "Please analyze this text: 'The quick brown fox jumps over the lazy dog. \
            This is a test sentence.' Tell me about its word and character count.",
        )
        .multi_turn(3)
        .await?;
    println!("Gemini: {}\n", response);

    // Example 2: Unit conversion
    println!("Example 2: Unit conversion");
    let response = agent
        .prompt("Convert 100 kilometers to miles. Also convert 75 fahrenheit to celsius.")
        .multi_turn(4)
        .await?;
    println!("Gemini: {}\n", response);

    // Example 3: Data storage and retrieval
    println!("Example 3: Data storage");
    let response = agent
        .prompt(
            "Store the following information: \
            key 'user_name' with value 'Alice', \
            key 'user_age' with value '30', \
            and key 'user_city' with value 'San Francisco'. \
            Then retrieve and show me all the stored keys.",
        )
        .multi_turn(6)
        .await?;
    println!("Gemini: {}\n", response);

    // Example 4: Retrieve stored data
    println!("Example 4: Data retrieval");
    let response = agent
        .prompt("What is the user's name and city that we stored earlier?")
        .multi_turn(4)
        .await?;
    println!("Gemini: {}\n", response);

    Ok(())
}
