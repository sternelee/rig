# Model Context Protocol (MCP) Examples

This directory contains examples demonstrating how to integrate Rig with the Model Context Protocol (MCP) for building agent applications with Claude Code, Gemini CLI, and other MCP-compatible tools.

## What is MCP?

The [Model Context Protocol (MCP)](https://spec.modelcontextprotocol.io/) is an open protocol that standardizes how applications provide context to LLMs. It enables:

- **Tool Integration**: Agents can call functions exposed by MCP servers
- **Resource Access**: Agents can read files, data, and other resources
- **Stateful Operations**: Tools can maintain state across multiple calls
- **Universal Compatibility**: Works with any LLM provider (Claude, Gemini, GPT, etc.)

## Available Examples

### 1. MCP with Claude (`mcp_with_claude.rs`)

Demonstrates integrating Claude (Anthropic) with an MCP server providing:
- Calculator operations (add, subtract, multiply, divide)
- Current time retrieval
- Stateful counter operations

**Run:**
```bash
export ANTHROPIC_API_KEY="your-key"
cargo run --example mcp_with_claude --features rmcp
```

### 2. MCP with Gemini (`mcp_with_gemini.rs`)

Shows Gemini integration with an MCP server offering:
- Text analysis (word count, character count, etc.)
- Unit conversion (distance, weight, temperature)
- Simple key-value data storage

**Run:**
```bash
export GEMINI_API_KEY="your-key"
cargo run --example mcp_with_gemini --features rmcp
```

### 3. Basic MCP Example (`rmcp.rs`)

Foundational example showing the core MCP integration pattern.

**Run:**
```bash
export OPENAI_API_KEY="your-key"
cargo run --example rmcp --features rmcp
```

## Quick Start

### 1. Add Dependencies

```toml
[dependencies]
rig-core = { version = "0.24", features = ["rmcp"] }
rmcp = { version = "0.8", features = [
    "client",
    "macros",
    "transport-streamable-http-client-reqwest",
] }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

### 2. Create an MCP Server

```rust
use rmcp::{RoleServer, ServerHandler, tool, tool_handler, tool_router};

#[derive(Clone)]
pub struct MyServer {
    tool_router: ToolRouter<MyServer>,
}

#[tool_router]
impl MyServer {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }

    #[tool(description = "Add two numbers")]
    fn add(&self, Parameters(args): Parameters<AddArgs>) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![
            Content::text((args.a + args.b).to_string())
        ]))
    }
}

#[tool_handler]
impl ServerHandler for MyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("My MCP server".to_string()),
        }
    }
}
```

### 3. Connect Your Agent

```rust
use rig::{client::CompletionClient, completion::Prompt, providers::anthropic};
use rmcp::ServiceExt;

// Connect to MCP server
let transport = rmcp::transport::StreamableHttpClientTransport::from_uri("http://localhost:3000");
let client_info = rmcp::model::ClientInfo {
    protocol_version: Default::default(),
    capabilities: Default::default(),
    client_info: rmcp::model::Implementation {
        name: "my-agent".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    },
};

let client = client_info.serve(transport).await?;
let tools = client.list_tools(Default::default()).await?.tools;

// Create agent with MCP tools
let agent = anthropic::Client::from_env()
    .agent(anthropic::CLAUDE_3_5_SONNET)
    .preamble("You are a helpful assistant with MCP tools.")
    .rmcp_tools(tools, client.peer().to_owned())
    .build();

// Use the agent
let response = agent
    .prompt("What is 2 + 2?")
    .multi_turn(3)
    .await?;
```

## Key Concepts

### Multi-Turn Prompting

MCP tools often require multiple LLM interactions. Use `.multi_turn(n)`:

```rust
let response = agent
    .prompt("Complex query requiring multiple tool calls")
    .multi_turn(5)  // Allow up to 5 turns
    .await?;
```

### Tool Definitions

The `#[tool]` macro automatically generates JSON schemas for your functions:

```rust
#[tool(description = "Convert temperature units")]
fn convert_temperature(
    &self,
    Parameters(args): Parameters<TempConversionArgs>,
) -> Result<CallToolResult, ErrorData> {
    // Implementation
}
```

### Stateful Tools

Share state across tool calls using `Arc<Mutex<T>>`:

```rust
#[derive(Clone)]
pub struct StatefulServer {
    counter: Arc<Mutex<i32>>,
    tool_router: ToolRouter<StatefulServer>,
}

#[tool(description = "Increment counter")]
async fn increment(&self) -> Result<CallToolResult, ErrorData> {
    let mut counter = self.counter.lock().await;
    *counter += 1;
    Ok(CallToolResult::success(vec![Content::text(counter.to_string())]))
}
```

### Error Handling

Return detailed errors to help the LLM understand what went wrong:

```rust
#[tool(description = "Divide numbers")]
fn divide(&self, Parameters(args): Parameters<DivideArgs>) -> Result<CallToolResult, ErrorData> {
    if args.b == 0.0 {
        return Err(ErrorData::invalid_params(
            "division_by_zero",
            Some(json!({"message": "Cannot divide by zero"})),
        ));
    }
    // ... implementation
}
```

## Provider Compatibility

MCP works with all Rig providers that support tools:

| Provider | Status | Example Usage |
|----------|--------|---------------|
| Anthropic (Claude) | ✅ Fully Supported | See `mcp_with_claude.rs` |
| Google (Gemini) | ✅ Fully Supported | See `mcp_with_gemini.rs` |
| OpenAI | ✅ Fully Supported | Use `openai::Client` with `.rmcp_tools()` |
| Cohere | ✅ Fully Supported | Use `cohere::Client` with `.rmcp_tools()` |
| Mistral | ✅ Fully Supported | Use `mistral::Client` with `.rmcp_tools()` |
| Other Providers | ✅ Supported | Any provider that supports function calling |

## Use Cases

### Code Assistance

Build code analysis, refactoring, and generation tools:

```rust
// Tools: analyze_code, suggest_improvements, generate_tests
agent.prompt("Analyze this function and suggest improvements").await?;
```

### CLI Integration

Expose command-line tools to LLMs:

```rust
// Tools: list_files, read_file, execute_command
agent.prompt("List all Python files in the current directory").await?;
```

### Database Access

Provide safe database query capabilities:

```rust
// Tools: query_database, get_schema, count_records
agent.prompt("How many active users do we have?").await?;
```

### API Integration

Connect to external services:

```rust
// Tools: fetch_weather, get_stock_price, send_email
agent.prompt("What's the weather in San Francisco?").await?;
```

## Best Practices

1. **Clear Descriptions**: Write detailed tool descriptions so the LLM knows when to use them
2. **Type Safety**: Use strongly-typed argument structures with `#[derive(Deserialize, JsonSchema)]`
3. **Error Messages**: Return helpful error messages that guide the LLM to retry correctly
4. **Stateless When Possible**: Prefer stateless tools for simplicity
5. **Multi-Turn**: Always use `.multi_turn()` for complex workflows
6. **Testing**: Test your MCP tools independently before integrating with LLMs

## Troubleshooting

### "Tool not found" errors

Ensure your MCP server is running and the client is connected:
```rust
let tools = client.list_tools(Default::default()).await?;
println!("Available tools: {:?}", tools.iter().map(|t| &t.name).collect::<Vec<_>>());
```

### LLM doesn't use the tool

- Make the tool description more specific
- Include examples in the agent preamble
- Increase `multi_turn` limit
- Check that tool parameters match what the LLM is trying to pass

### Connection errors

- Verify the server URI is correct
- Check firewall settings
- Ensure required transport features are enabled in rmcp

## Additional Resources

- [Full MCP Integration Guide](../MCP_INTEGRATION_GUIDE.md)
- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [rmcp Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Rig Documentation](https://docs.rig.rs)

## Contributing

Have a cool MCP integration example? We'd love to see it! Please:

1. Open an issue with your idea
2. Submit a pull request with your example
3. Share your use case with the community

## License

These examples are part of Rig and are MIT licensed.
