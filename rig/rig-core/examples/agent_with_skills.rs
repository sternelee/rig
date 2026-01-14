use anyhow::Result;
use rig::{
    agent::{AgentBuilder, SimpleSkill, Skill, SkillComponents},
    client::{CompletionClient, ProviderClient},
    completion::{Prompt, ToolDefinition},
    providers::openai::{self, Client},
    tool::{Tool, ToolDyn},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

// ============================================================================
// Example 1: Math skill with calculator tools
// ============================================================================

#[derive(Deserialize)]
struct AddArgs {
    x: f64,
    y: f64,
}

#[derive(Debug, thiserror::Error)]
#[error("Math error")]
struct MathError;

#[derive(Serialize, Deserialize)]
struct AddTool;

impl Tool for AddTool {
    const NAME: &'static str = "add";
    type Error = MathError;
    type Args = AddArgs;
    type Output = f64;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "add".to_string(),
            description: "Add two numbers together".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": {
                        "type": "number",
                        "description": "The first number"
                    },
                    "y": {
                        "type": "number",
                        "description": "The second number"
                    }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(args.x + args.y)
    }
}

#[derive(Deserialize)]
struct MultiplyArgs {
    x: f64,
    y: f64,
}

#[derive(Serialize, Deserialize)]
struct MultiplyTool;

impl Tool for MultiplyTool {
    const NAME: &'static str = "multiply";
    type Error = MathError;
    type Args = MultiplyArgs;
    type Output = f64;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "multiply".to_string(),
            description: "Multiply two numbers together".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": {
                        "type": "number",
                        "description": "The first number"
                    },
                    "y": {
                        "type": "number",
                        "description": "The second number"
                    }
                },
                "required": ["x", "y"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(args.x * args.y)
    }
}

// Create a math skill using SimpleSkill builder
fn create_math_skill() -> SimpleSkill {
    SimpleSkill::builder()
        .name("math")
        .description("Mathematical calculation capabilities")
        .preamble(
            "You have access to mathematical tools for addition and multiplication. \
             Use them to perform calculations when needed.",
        )
        .tool(Box::new(AddTool) as Box<dyn ToolDyn>)
        .tool(Box::new(MultiplyTool) as Box<dyn ToolDyn>)
        .context_document(
            "math_context",
            "When performing calculations, always show your work step by step.",
        )
        .build()
}

// ============================================================================
// Example 2: Custom skill with the Skill trait
// ============================================================================

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
                "You are a friendly assistant. Always greet users warmly and \
                 maintain a positive, helpful tone throughout the conversation."
                    .to_string(),
            ),
            context_documents: vec![],
        }
    }
}

// ============================================================================
// Example usage
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create OpenAI client
    let openai_client = Client::from_env();
    let model = openai_client.completion_model(openai::GPT_4O_MINI);

    println!("=== Example 1: Agent with Math Skill ===\n");

    // Create an agent with the math skill
    let math_agent = AgentBuilder::new(model.clone())
        .preamble("You are a helpful math assistant.")
        .skill(create_math_skill())
        .build();

    let response = math_agent
        .prompt("What is 15 plus 27, then multiply the result by 3?")
        .await?;

    println!("Math Agent Response: {}\n", response);

    println!("=== Example 2: Agent with Greeting Skill ===\n");

    // Create an agent with the greeting skill
    let greeting_agent = AgentBuilder::new(model.clone())
        .skill(GreetingSkill)
        .build();

    let response = greeting_agent
        .prompt("Hello! Can you help me today?")
        .await?;

    println!("Greeting Agent Response: {}\n", response);

    println!("=== Example 3: Agent with Multiple Skills ===\n");

    // Create an agent with multiple skills
    // Note: We need to create the agent in a specific way since skills are consumed
    let multi_skill_agent = AgentBuilder::new(model)
        .preamble("You are a versatile assistant.")
        .skill(GreetingSkill)
        .skill(create_math_skill())
        .build();

    let response = multi_skill_agent
        .prompt("Hello! Can you calculate 10 times 5 for me?")
        .await?;

    println!("Multi-Skill Agent Response: {}\n", response);

    Ok(())
}
