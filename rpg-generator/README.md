# rpg-generator

> Generate codebases from natural language descriptions using Repository Planning Graphs

Part of the [RPG-Rust](https://github.com/microsoft/rpg-rust) workspace implementing Microsoft's [ZeroRepo](https://arxiv.org/abs/2502.02084) paper.

## Overview

The generator performs the inverse operation of `rpg-encoder`:

- **rpg-encoder**: Codebase → RPG (extract structure)
- **rpg-generator**: Description → RPG → Code (generate from intent)

## Architecture

The generator uses a four-phase pipeline:

1. **Phase 1 (Property Level)**: Description → FeatureTree + ComponentPlan
2. **Phase 2 (Implementation Level)**: Components → RepoSkeleton + TaskPlan
3. **Phase 3 (Code Generation)**: Tasks → Generated code via TDD loop
4. **Phase 4 (Verification)**: Verify generated code matches intent

## Installation

```toml
[dependencies]
rpg-generator = "0.1"

[features]
default = ["llm", "opencode"]
llm = []        # LLM API integration
opencode = []   # OpenCode CLI agent
trae = []       # Trae CLI agent
claude = []     # Claude CLI agent
all-agents = ["opencode", "trae", "claude"]
```

## Usage

```rust
use rpg_generator::{RpgGenerator, GenerationRequest, TargetLanguage, LlmConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = LlmConfig::new(std::env::var("OPENAI_API_KEY")?);
    let generator = RpgGenerator::new(config);
    
    let request = GenerationRequest::new(
        "A REST API for task management with CRUD operations",
        TargetLanguage::Rust,
    );
    
    let output = generator.generate(request).await?;
    println!("Generated {} files with {} tasks completed",
        output.total_files(), output.completed_tasks());
    
    Ok(())
}
```

## Features

- **Multi-Agent Support**: OpenCode, Trae, Claude CLI integration
- **TDD Loop**: Test-driven development with automatic test generation
- **Checkpoint/Restore**: Recovery and debugging support
- **Multi-Dimensional Verification**: Tests, lint, type checking

## License

Apache License, Version 2.0
