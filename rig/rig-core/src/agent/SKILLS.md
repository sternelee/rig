# Agent Skills

Agent Skills provide a higher-level abstraction for composing and reusing agent capabilities in Rig.

## What is a Skill?

A Skill is a reusable package that can contain:
- **Tools**: Functions the agent can call
- **Preamble**: System prompt additions
- **Context documents**: Static knowledge the agent can reference

Skills make it easy to create modular, composable capabilities that can be shared across multiple agents.

## Usage

### Creating a Simple Skill

Use the `SimpleSkill` builder to create skills programmatically:

```rust
use rig::agent::SimpleSkill;
use rig::tool::ToolDyn;

let math_skill = SimpleSkill::builder()
    .name("math")
    .description("Mathematical calculation capabilities")
    .preamble("You have access to mathematical tools for calculations.")
    .tool(Box::new(AddTool) as Box<dyn ToolDyn>)
    .tool(Box::new(MultiplyTool) as Box<dyn ToolDyn>)
    .context_document("math_help", "Always show your work step by step.")
    .build();
```

### Creating a Custom Skill

Implement the `Skill` trait for more complex use cases:

```rust
use rig::agent::{Skill, SkillComponents};

struct GreetingSkill;

impl Skill for GreetingSkill {
    fn name(&self) -> String {
        "greeting".to_string()
    }

    fn description(&self) -> String {
        "Provides friendly greeting capabilities".to_string()
    }

    fn into_components(self) -> SkillComponents {
        SkillComponents {
            tools: vec![],
            preamble: Some(
                "You are a friendly assistant. Always greet users warmly."
                    .to_string()
            ),
            context_documents: vec![],
        }
    }
}
```

### Adding Skills to Agents

Add skills to agents using the `.skill()` method:

```rust
use rig::{agent::AgentBuilder, client::CompletionClient};
use rig::providers::openai;

let openai_client = openai::Client::from_env();
let model = openai_client.completion_model(openai::GPT_4O_MINI);

// Add a single skill
let agent = AgentBuilder::new(model.clone())
    .preamble("You are a helpful assistant.")
    .skill(math_skill)
    .build();

// Add multiple skills (chain them)
let multi_skill_agent = AgentBuilder::new(model)
    .skill(GreetingSkill)
    .skill(math_skill)
    .build();
```

## Design Notes

### Skills Are Consumed

Skills are consumed (moved) when added to an agent because tools cannot be cloned. This means:
- Each skill instance can only be added to one agent
- To reuse a skill pattern, create a factory function that returns new instances

```rust
fn create_math_skill() -> SimpleSkill {
    SimpleSkill::builder()
        .name("math")
        // ... configuration
        .build()
}

// Use in multiple agents
let agent1 = AgentBuilder::new(model1).skill(create_math_skill()).build();
let agent2 = AgentBuilder::new(model2).skill(create_math_skill()).build();
```

### Builder Pattern

- `AgentBuilder::skill()` transitions to `AgentBuilderSimple`
- `AgentBuilderSimple::skill()` returns `Self` for chaining
- This matches the existing pattern for `.tool()` methods

### Preamble Merging

When a skill with a preamble is added to an agent:
- If the agent already has a preamble, the skill's preamble is appended with a newline separator
- If the agent has no preamble (or an empty one), the skill's preamble becomes the agent's preamble
- This allows skills to compose their prompts naturally

## Examples

See `examples/agent_with_skills.rs` for a complete working example demonstrating:
- Creating skills with tools
- Creating skills with just prompts
- Combining multiple skills in one agent
- Custom skill implementations

## Related

- [Tool trait](../tool/mod.rs) - Lower-level tool abstraction
- [AgentBuilder](./builder.rs) - Builder for creating agents
- [Agent documentation](./mod.rs) - Complete agent documentation
