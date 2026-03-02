//! Prompts for LLM interactions.

pub const FEATURE_EXTRACTION_PROMPT: &str = r#"
You are an expert software architect analyzing a project description.

## Task
Extract a hierarchical feature tree from the project description.

## Constraints
- Output 1-8 top-level feature categories
- Each category can have subcategories
- Each feature should be a verb + noun phrase (e.g., "manage users", "validate input")
- Features should be specific and actionable
- NO vague terms: Core, Misc, Utils, Common, General, Shared, Other

## Output Format (STRICT JSON)
{
  "root_name": "project_name",
  "categories": [
    {
      "name": "CategoryName",
      "description": "Brief description",
      "features": ["feature 1", "feature 2"],
      "subcategories": [
        {
          "name": "SubcategoryName",
          "features": ["feature 3"]
        }
      ]
    }
  ]
}

Project Description:
{description}
"#;

pub const FEATURE_REFACTORING_PROMPT: &str = r#"
You are an expert software architect organizing features into components.

## Task
Group the following features into logical components (top-level directories).

## Constraints
- Output 3-10 components
- Each component should be a cohesive functional area
- Component names should use dot notation (e.g., "auth.login", "game.physics")
- Each feature must be assigned to exactly one component
- NO vague component names: Core, Misc, Utils, Common, General, Shared

## Output Format (STRICT JSON)
{
  "components": [
    {
      "name": "component.name",
      "description": "Brief description of this component's responsibility",
      "features": ["feature 1", "feature 2"]
    }
  ]
}

Features:
{features}
"#;

pub const SKELETON_DESIGN_PROMPT: &str = r#"
You are an expert software architect designing a project structure.

## Task
Create a file skeleton for a {language} project with the following components.

## Constraints
- Follow {language} project conventions
- Each component should have its own directory
- Include a shared directory for common types/utilities
- File paths should be relative to project root

## Output Format (STRICT JSON)
{
  "directories": ["src/", "src/component_a/", "src/shared/"],
  "files": [
    {
      "path": "src/component_a/mod.rs",
      "purpose": "Module exports for component_a",
      "component": "component_a"
    }
  ],
  "entry_point": "src/main.rs"
}

Components:
{components}
"#;

pub const INTERFACE_DESIGN_PROMPT: &str = r#"
You are an expert {language} developer designing interfaces.

## Task
Design the public interfaces (functions, classes, types) for a file.

## Constraints
- Follow {language} naming conventions
- Include type annotations
- Write brief docstrings
- Keep functions focused and single-purpose

## Output Format (STRICT JSON)
{
  "imports": ["use std::collections::HashMap;"],
  "units": [
    {
      "name": "function_name",
      "kind": "function",
      "signature": "pub fn function_name(param: Type) -> Result<Output, Error>",
      "docstring": "Brief description of what this function does",
      "features": ["feature_1", "feature_2"]
    }
  ]
}

File: {file_path}
Component: {component}
Features to implement:
{features}
"#;
