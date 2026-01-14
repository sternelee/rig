//! Module defining the Skill trait and related structures.
//!
//! A Skill is a higher-level abstraction that represents a reusable capability
//! that can be added to an agent. Skills can provide:
//! - A set of tools
//! - A preamble (system prompt addition)
//! - Context documents
//!
//! Skills make it easy to compose and reuse common agent capabilities across
//! different agents.

use crate::{
    completion::Document,
    tool::ToolDyn,
};
use std::collections::HashMap;

/// Trait representing a reusable agent skill.
///
/// A skill encapsulates a set of related tools, a preamble, and context documents
/// that can be added to an agent to give it specific capabilities.
///
/// Note: Skills are consumed when added to an agent, as tools cannot be cloned.
///
/// # Example
/// ```rust,ignore
/// use rig::agent::Skill;
/// use rig::completion::Document;
/// use rig::tool::ToolDyn;
///
/// struct CalculatorSkill;
///
/// impl Skill for CalculatorSkill {
///     fn name(&self) -> String {
///         "calculator".to_string()
///     }
///
///     fn description(&self) -> String {
///         "Provides mathematical calculation capabilities".to_string()
///     }
///
///     fn into_components(self) -> SkillComponents {
///         SkillComponents {
///             tools: vec![
///                 // Add calculator tools here
///             ],
///             preamble: Some("You have access to calculator tools.".to_string()),
///             context_documents: vec![],
///         }
///     }
/// }
/// ```
pub trait Skill: Send + Sync {
    /// Returns the name of the skill.
    fn name(&self) -> String;

    /// Returns a description of what the skill provides.
    fn description(&self) -> String {
        String::new()
    }

    /// Consumes the skill and returns its components.
    /// This is required because tools cannot be cloned.
    fn into_components(self) -> SkillComponents;
}

/// The components that make up a skill.
pub struct SkillComponents {
    /// The tools provided by this skill.
    pub tools: Vec<Box<dyn ToolDyn>>,
    /// The preamble (system prompt addition) for this skill.
    pub preamble: Option<String>,
    /// The static context documents for this skill.
    pub context_documents: Vec<Document>,
}

/// A simple skill implementation that can be built programmatically.
///
/// # Example
/// ```rust
/// use rig::agent::SimpleSkill;
///
/// let skill = SimpleSkill::builder()
///     .name("my_skill")
///     .description("A custom skill")
///     .preamble("You are an expert in this domain.")
///     .build();
/// ```
pub struct SimpleSkill {
    name: String,
    description: String,
    tools: Vec<Box<dyn ToolDyn>>,
    preamble: Option<String>,
    context_documents: Vec<Document>,
}

impl SimpleSkill {
    /// Creates a new builder for SimpleSkill.
    pub fn builder() -> SimpleSkillBuilder {
        SimpleSkillBuilder::new()
    }
}

impl Skill for SimpleSkill {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn description(&self) -> String {
        self.description.clone()
    }

    fn into_components(self) -> SkillComponents {
        SkillComponents {
            tools: self.tools,
            preamble: self.preamble,
            context_documents: self.context_documents,
        }
    }
}

/// Builder for creating SimpleSkill instances.
pub struct SimpleSkillBuilder {
    name: Option<String>,
    description: String,
    tools: Vec<Box<dyn ToolDyn>>,
    preamble: Option<String>,
    context_documents: Vec<Document>,
}

impl SimpleSkillBuilder {
    fn new() -> Self {
        Self {
            name: None,
            description: String::new(),
            tools: Vec::new(),
            preamble: None,
            context_documents: Vec::new(),
        }
    }

    /// Sets the name of the skill.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the description of the skill.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Adds a tool to the skill.
    pub fn tool(mut self, tool: Box<dyn ToolDyn>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Adds multiple tools to the skill.
    pub fn tools(mut self, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Sets the preamble for the skill.
    pub fn preamble(mut self, preamble: impl Into<String>) -> Self {
        self.preamble = Some(preamble.into());
        self
    }

    /// Adds a context document to the skill.
    pub fn context_document(mut self, id: impl Into<String>, text: impl Into<String>) -> Self {
        self.context_documents.push(Document {
            id: id.into(),
            text: text.into(),
            additional_props: HashMap::new(),
        });
        self
    }

    /// Adds multiple context documents to the skill.
    pub fn context_documents(mut self, documents: Vec<Document>) -> Self {
        self.context_documents.extend(documents);
        self
    }

    /// Builds the SimpleSkill.
    ///
    /// # Panics
    /// Panics if the name is not set.
    pub fn build(self) -> SimpleSkill {
        SimpleSkill {
            name: self.name.expect("Skill name is required"),
            description: self.description,
            tools: self.tools,
            preamble: self.preamble,
            context_documents: self.context_documents,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::Tool;
    use crate::completion::ToolDefinition;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Deserialize)]
    struct TestToolArgs {
        value: String,
    }

    #[derive(Debug, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    #[derive(Serialize, Deserialize)]
    struct TestTool;

    impl Tool for TestTool {
        const NAME: &'static str = "test_tool";
        type Error = TestError;
        type Args = TestToolArgs;
        type Output = String;

        async fn definition(&self, _prompt: String) -> ToolDefinition {
            ToolDefinition {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "value": {
                            "type": "string",
                            "description": "A test value"
                        }
                    },
                    "required": ["value"]
                }),
            }
        }

        async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
            Ok(args.value)
        }
    }

    #[test]
    fn test_simple_skill_builder() {
        let skill = SimpleSkill::builder()
            .name("test_skill")
            .description("A test skill")
            .preamble("Test preamble")
            .context_document("doc1", "Test document")
            .build();

        assert_eq!(skill.name(), "test_skill");
        assert_eq!(skill.description(), "A test skill");
        
        let components = skill.into_components();
        assert_eq!(components.preamble, Some("Test preamble".to_string()));
        assert_eq!(components.context_documents.len(), 1);
    }

    #[test]
    fn test_simple_skill_with_tools() {
        let tool: Box<dyn ToolDyn> = Box::new(TestTool);
        let skill = SimpleSkill::builder()
            .name("test_skill")
            .tool(tool)
            .build();

        assert_eq!(skill.name(), "test_skill");
        
        let components = skill.into_components();
        assert_eq!(components.tools.len(), 1);
    }

    #[test]
    #[should_panic(expected = "Skill name is required")]
    fn test_simple_skill_builder_without_name() {
        SimpleSkill::builder().build();
    }
}
