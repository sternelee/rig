# Model Context Protocol (MCP) Integration Guide

This guide explains how to integrate Rig with the Model Context Protocol (MCP) to enable agent tools and capabilities from MCP servers like Claude Code, Gemini CLI, and others.

## What is MCP?

The Model Context Protocol (MCP) is an open protocol that standardizes how applications provide context to LLMs. It enables:

- **Tool Integration**: Agents can use tools exposed by MCP servers
- **Resource Access**: Agents can access files, data, and other resources
- **Stateful Operations**: Tools can maintain state across multiple calls
- **Cross-Platform Support**: Works with various LLM providers (Claude, Gemini, GPT, etc.)

## Features

Rig's MCP integration via the `rmcp` feature provides:

- ✅ Full MCP protocol support through the official Rust MCP SDK
- ✅ Seamless integration with all Rig providers (Anthropic, Gemini, OpenAI, etc.)
- ✅ Support for MCP tools, resources, and prompts
- ✅ HTTP and stdio transport support
- ✅ Type-safe tool definitions
- ✅ Async/await support

## Installation

Add Rig with the `rmcp` feature to your `Cargo.toml`:

```toml
[dependencies]
rig-core = { version = "0.24", features = ["rmcp"] }
rmcp = { version = "0.8", features = ["client", "transport-streamable-http-client-reqwest"] }
```

## Quick Start

### 1. Create an MCP Server

```rust
use rmcp::{
    RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
};

#[derive(Clone)]
pub struct MyMcpServer {
    tool_router: ToolRouter<MyMcpServer>,
}

#[tool_router]
impl MyMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Add two numbers")]
    fn add(&self, Parameters(args): Parameters<AddArgs>) -> Result<CallToolResult, ErrorData> {
        let result = args.a + args.b;
        Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
    }
}

#[tool_handler]
impl ServerHandler for MyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Calculator MCP server".to_string()),
        }
    }
}
```

### 2. Connect Your Agent to MCP

```rust
use rig::{completion::Prompt, providers::anthropic};
use rmcp::ServiceExt;

// Connect to MCP server
let transport = rmcp::transport::StreamableHttpClientTransport::from_uri("http://localhost:3000");
let client_info = ClientInfo {
    protocol_version: Default::default(),
    capabilities: ClientCapabilities::default(),
    client_info: Implementation {
        name: "my-agent".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    },
};

let client = client_info.serve(transport).await?;
let tools = client.list_tools(Default::default()).await?.tools;

// Create agent with MCP tools
let anthropic_client = anthropic::Client::from_env();
let agent = anthropic_client
    .agent(anthropic::CLAUDE_3_5_SONNET)
    .preamble("You are a helpful assistant with access to MCP tools.")
    .rmcp_tools(tools, client.peer().to_owned())
    .build();

// Use the agent
let response = agent
    .prompt("What is 2 + 2?")
    .multi_turn(3)
    .await?;
```

## Examples

### Claude Code Integration

See `examples/mcp_with_claude.rs` for a complete example showing:
- Calculator operations
- Time queries
- Stateful counter operations
- Multi-step reasoning with tools

Run with:
```bash
export ANTHROPIC_API_KEY="your-key"
cargo run --example mcp_with_claude --features rmcp
```

### Gemini CLI Integration

See `examples/mcp_with_gemini.rs` for a complete example showing:
- Text analysis tools
- Unit conversion
- Key-value data storage
- Complex multi-tool workflows

Run with:
```bash
export GEMINI_API_KEY="your-key"
cargo run --example mcp_with_gemini --features rmcp
```

### Basic MCP Example

See `examples/rmcp.rs` for the foundational MCP integration example.

## Agent Builder Methods

When using MCP with Rig agents, you have several methods available:

### `rmcp_tool(tool, client)`

Add a single MCP tool to the agent:

```rust
agent_builder.rmcp_tool(tool, client)
```

### `rmcp_tools(tools, client)`

Add multiple MCP tools to the agent:

```rust
agent_builder.rmcp_tools(tools, client)
```

## Transport Options

### HTTP Transport (Recommended)

```rust
use rmcp::transport::StreamableHttpClientTransport;

let transport = StreamableHttpClientTransport::from_uri("http://localhost:3000");
```

### Stdio Transport

For command-line MCP servers:

```rust
use rmcp::transport::StdioClientTransport;

let transport = StdioClientTransport::new("mcp-server-command");
```

## Tool Types

MCP supports several types of capabilities:

### Tools (Functions)

Functions that the LLM can call:

```rust
#[tool(description = "Get weather for a location")]
fn get_weather(&self, Parameters(args): Parameters<WeatherArgs>) -> Result<CallToolResult, ErrorData> {
    // Implementation
}
```

### Resources

Data sources that can be read:

```rust
async fn read_resource(
    &self,
    ReadResourceRequestParam { uri }: ReadResourceRequestParam,
    _: RequestContext<RoleServer>,
) -> Result<ReadResourceResult, ErrorData> {
    // Return resource content
}
```

### Prompts

Reusable prompt templates (coming soon to Rig).

## Best Practices

### 1. Use Multi-Turn Prompting

MCP tools often require multiple interactions:

```rust
agent.prompt("Complex query requiring multiple tool calls")
    .multi_turn(5)  // Allow up to 5 turns
    .await?
```

### 2. Provide Clear Preambles

Help the agent understand how to use MCP tools:

```rust
agent_builder.preamble(
    "You are a helpful assistant with access to the following MCP tools: \
    calculator, weather, and database access. Use these tools to answer questions accurately."
)
```

### 3. Handle Errors Gracefully

MCP tools can fail; implement proper error handling:

```rust
#[tool(description = "Divide two numbers")]
fn divide(&self, Parameters(args): Parameters<DivideArgs>) -> Result<CallToolResult, ErrorData> {
    if args.b == 0.0 {
        return Err(ErrorData::invalid_params(
            "division_by_zero",
            Some(json!({"message": "Cannot divide by zero"})),
        ));
    }
    Ok(CallToolResult::success(vec![Content::text((args.a / args.b).to_string())]))
}
```

### 4. Use Stateful Tools When Needed

MCP supports stateful operations through shared state:

```rust
#[derive(Clone)]
pub struct StatefulServer {
    state: Arc<Mutex<AppState>>,
    tool_router: ToolRouter<StatefulServer>,
}

#[tool(description = "Update state")]
async fn update_state(&self) -> Result<CallToolResult, ErrorData> {
    let mut state = self.state.lock().await;
    state.counter += 1;
    Ok(CallToolResult::success(vec![Content::text(state.counter.to_string())]))
}
```

## Provider Compatibility

MCP works with all Rig providers:

| Provider | Supported | Example |
|----------|-----------|---------|
| Anthropic (Claude) | ✅ | `mcp_with_claude.rs` |
| Google (Gemini) | ✅ | `mcp_with_gemini.rs` |
| OpenAI | ✅ | Use `openai::Client` with `.rmcp_tools()` |
| Cohere | ✅ | Use `cohere::Client` with `.rmcp_tools()` |
| Mistral | ✅ | Use `mistral::Client` with `.rmcp_tools()` |
| All others | ✅ | Any provider supporting tools |

## Common Use Cases

### 1. Code Assistance (Claude Code)

```rust
// Connect to Claude Code MCP server
let agent = anthropic_client
    .agent(CLAUDE_3_5_SONNET)
    .rmcp_tools(code_tools, client.peer().to_owned())
    .build();

agent.prompt("Refactor this function to use async/await").await?;
```

### 2. CLI Tools (Gemini CLI)

```rust
// Connect to CLI tool MCP server
let agent = gemini_client
    .agent(GEMINI_2_0_FLASH)
    .rmcp_tools(cli_tools, client.peer().to_owned())
    .build();

agent.prompt("List files in the current directory").await?;
```

### 3. Database Access

```rust
// MCP server with database tools
let agent = openai_client
    .agent("gpt-4")
    .rmcp_tools(db_tools, client.peer().to_owned())
    .build();

agent.prompt("Query the users table for active accounts").await?;
```

## Advanced Topics

### Custom Transport

Implement your own transport for special use cases:

```rust
use rmcp::transport::Transport;

struct CustomTransport {
    // Your implementation
}

impl Transport for CustomTransport {
    // Implement required methods
}
```

### Tool Discovery

List available tools at runtime:

```rust
let tools = client.list_tools(Default::default()).await?.tools;
for tool in tools {
    println!("{}: {}", tool.name, tool.description.unwrap_or_default());
}
```

### Dynamic Tool Addition

Add tools dynamically based on runtime conditions:

```rust
let mut agent_builder = client.agent(MODEL);

if should_add_calculator {
    agent_builder = agent_builder.rmcp_tool(calculator_tool, client.peer().to_owned());
}

let agent = agent_builder.build();
```

## Troubleshooting

### Connection Issues

If you can't connect to an MCP server:

1. Ensure the server is running
2. Check the URI is correct
3. Verify firewall settings
4. Check server logs

### Tool Not Called

If the LLM doesn't use your tool:

1. Make sure the description is clear and specific
2. Check that the tool name is descriptive
3. Use multi-turn prompting
4. Provide examples in the preamble

### Type Errors

If you get serialization errors:

1. Ensure your request types derive `Deserialize` and `JsonSchema`
2. Check that parameter names match the schema
3. Validate JSON schema compatibility

## Resources

- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [rmcp Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Rig Documentation](https://docs.rig.rs)
- [Example Repository](https://github.com/0xPlaygrounds/rig/tree/main/rig-core/examples)

## Contributing

We welcome contributions! If you've built interesting MCP integrations or have suggestions for improvements, please:

1. Open an issue on [GitHub](https://github.com/0xPlaygrounds/rig/issues)
2. Submit a pull request
3. Share your examples with the community

## License

Rig is MIT licensed. See the LICENSE file for details.
