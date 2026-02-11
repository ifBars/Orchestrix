# Skills Guide

Complete guide to the Orchestrix skills system - an MCP-compatible extension mechanism for agent capabilities.

## Table of Contents

- [Overview](#overview)
- [Skill Structure](#skill-structure)
- [Built-in Skills](#built-in-skills)
- [Creating Custom Skills](#creating-custom-skills)
- [Using Skills](#using-skills)
- [MCP Compatibility](#mcp-compatibility)

## Overview

Skills are modular extensions that provide specialized capabilities to AI agents. They are:

- **MCP-compatible** - Follow Model Context Protocol standards
- **Self-contained** - Each skill is a directory with documentation and metadata
- **Discoverable** - Automatically loaded from `.agents/skills/`
- **Versioned** - Support versioning for updates
- **Human-in-the-loop safe** - Extend capabilities without bypassing review and visibility expectations

## Skill Structure

A skill is a directory containing:

```
.agents/skills/my-skill/
├── SKILL.md          # Main documentation and metadata
└── [other files]     # Optional: prompts, templates, etc.
```

### SKILL.md Format

Skills use YAML frontmatter with Markdown content:

```markdown
---
name: my-skill
description: Brief description of what this skill does
version: 1.0.0
tags: [rust, api, testing]
---

# My Skill

## When to Use This Skill

- Specific use case 1
- Specific use case 2

## Guidelines

Detailed instructions for the AI agent...

## Examples

```rust
// Code example
```

## References

- [Link to docs](https://example.com)
```

### Required Frontmatter Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Unique identifier (kebab-case) |
| `description` | string | One-line summary |
| `version` | string | SemVer version |

### Optional Frontmatter Fields

| Field | Type | Description |
|-------|------|-------------|
| `tags` | string[] | Categories for filtering |
| `install_command` | string | Command to install dependencies |
| `url` | string | Link to external documentation |

## Built-in Skills

Orchestrix includes several built-in skills:

### context-driven-development

**Purpose**: Context-driven development methodology

**When to use**:
- Working with Conductor's context artifacts
- Managing product.md, tech-stack.md, workflow.md
- Establishing project context

**Location**: `.agents/skills/context-driven-development/`

**Key concepts**:
- Context → Spec → Plan → Implement workflow
- Artifact relationships (product.md, tech-stack.md, workflow.md)
- Living documentation principles

### subagent-driven-development

**Purpose**: Multi-agent execution patterns

**When to use**:
- Breaking tasks into parallel sub-agents
- Managing spec review and implementation workflow
- Code quality reviews

**Location**: `.agents/skills/subagent-driven-development/`

**Key concepts**:
- Planner → Spec Reviewer → Implementer → Code Quality Reviewer
- Spec-driven development
- Parallel execution strategies

### agentic-eval

**Purpose**: Self-critique and reflection patterns

**When to use**:
- Implementing evaluator-optimizer pipelines
- Rubric-based evaluation systems
- Iterative improvement workflows

**Location**: `.agents/skills/agentic-eval/`

**Key concepts**:
- LLM-as-judge patterns
- Self-critique loops
- Quality evaluation frameworks

### minimax

**Purpose**: MiniMax API integration

**When to use**:
- Chinese LLM chat
- Text-to-speech
- AI video generation

**Location**: `.agents/skills/minimax/`

### frontend-design

**Purpose**: Create distinctive frontend interfaces

**When to use**:
- Building web components
- Creating polished UIs
- Avoiding generic AI aesthetics

**Location**: `.agents/skills/frontend-design/`

## Creating Custom Skills

### Step 1: Create Skill Directory

```bash
mkdir -p .agents/skills/my-custom-skill
```

### Step 2: Write SKILL.md

```markdown
---
name: my-custom-skill
description: Guides for working with MyLibrary API
tags: [api, http, mylibrary]
version: 1.0.0
---

# MyLibrary API Guide

## When to Use This Skill

- Building integrations with MyLibrary
- Handling API authentication
- Working with webhooks

## Setup

1. Install dependencies:
   ```bash
   bun add mylibrary-sdk
   ```

2. Configure environment:
   ```bash
   export MYLIBRARY_API_KEY=your_key_here
   ```

## Patterns

### Making API Calls

```typescript
import { MyLibraryClient } from "mylibrary-sdk";

const client = new MyLibraryClient({
  apiKey: process.env.MYLIBRARY_API_KEY,
});

const result = await client.items.list();
```

### Error Handling

```typescript
try {
  const result = await client.items.create(data);
} catch (error) {
  if (error.code === "RATE_LIMITED") {
    // Retry with backoff
  }
}
```

## Best Practices

- Always validate API responses
- Use pagination for list endpoints
- Cache frequently accessed data

## Resources

- [API Documentation](https://docs.mylibrary.com)
- [SDK Reference](https://sdk.mylibrary.com)
```

### Step 3: Test Your Skill

1. Save the file
2. Restart Orchestrix (or wait for hot reload)
3. Search for your skill in the Skills panel
4. Use it in a task

### Step 4: Version Your Skill

Update the `version` field when making changes:

```markdown
---
version: 1.1.0  # Increment for updates
---
```

## Using Skills

### Via UI

1. Open the application
2. Navigate to Settings → Skills
3. Browse or search for skills
4. Click a skill to view its documentation

### Via Task Prompts

Reference skills in your prompts:

```
Create a React component using the frontend-design skill.
Follow the patterns in the context-driven-development skill.
```

The AI will automatically load and use the relevant skill documentation.

### Via API

```typescript
// List all available skills
const skills = await invoke<SkillCatalogItem[]>("list_available_skills");

// Search for specific skills
const results = await invoke<SkillCatalogItem[]>("search_skills", {
  query: "rust api",
  limit: 10,
});

// Import Context7 skill
await invoke("import_context7_skill", {
  libraryName: "tokio",
  query: "async runtime",
});

// Add custom skill
await invoke("add_custom_skill", {
   skill: {
     id: "my-custom-skill",
     title: "My Custom Skill",
     description: "...",
     install_command: "bun add my-package",
     url: "https://example.com",
     tags: ["custom"],
   },
});
```

## Skill Discovery

Skills are automatically discovered from multiple sources:

### 1. Built-in Skills

Located in `.agents/skills/` and bundled with the application.

### 2. Custom Skills

User-created skills stored in the database.

### 3. Context7 Skills

Dynamically imported from Context7 documentation:

```typescript
// Import documentation for any Context7 library
await invoke("import_context7_skill", {
  libraryName: "react",
  query: "hooks",
});
```

### 4. Vercel AI Skills

Import skills from Vercel's AI SDK:

```typescript
await invoke("import_vercel_skill", {
  slug: "skill-name",
});
```

## MCP Compatibility

Skills are designed to be **MCP-compatible** (Model Context Protocol):

### What is MCP?

MCP is an open protocol for model context exchange, enabling:
- Reusable skills across different AI applications
- Standardized tool definitions
- External skill providers

### MCP Features in Orchestrix

1. **Tool Definitions**: Skills can define tools with JSON schemas
2. **Resource Templates**: Access to external resources
3. **Sampling**: Request completions from the model
4. **Roots**: Workspace and file access

### Future MCP Integration

Planned features:
- Connect to external MCP servers
- Skill marketplace integration
- Cross-application skill sharing

## Best Practices

### Skill Design

1. **Be specific**: Focus on one domain or use case
2. **Include examples**: Show real code patterns
3. **Explain when**: Clearly state when to use the skill
4. **Keep updated**: Version and maintain your skills
5. **Preserve transparency**: Skills must not encourage hidden side effects or skipped review gates
6. **Keep users involved**: Include patterns that support inspectable outputs and explicit approvals

### Documentation

1. **Start with frontmatter**: Always include required fields
2. **Use clear headings**: Organize with h2/h3 sections
3. **Add code examples**: Show practical usage
4. **Link resources**: Reference external documentation

### Maintenance

1. **Version changes**: Use semantic versioning
2. **Update regularly**: Keep documentation current
3. **Test examples**: Ensure code samples work
4. **Collect feedback**: Improve based on usage

## Skill Catalog Schema

```typescript
interface SkillCatalogItem {
  id: string;              // Unique identifier
  title: string;           // Display name
  description: string;     // One-line summary
  install_command: string; // Setup command (optional)
  url: string;            // External link (optional)
  source: string;         // "built-in", "custom", "context7", etc.
  tags: string[];         // Categories
  is_custom: boolean;     // User-created flag
}
```

## Troubleshooting

### Skill Not Appearing

1. Check the skill directory name matches the `name` field
2. Verify SKILL.md is valid Markdown
3. Ensure frontmatter is properly formatted
4. Restart the application

### Skill Not Loading

1. Check the skill directory is in `.agents/skills/`
2. Verify file permissions
3. Check for YAML syntax errors in frontmatter

### Context7 Skill Import Fails

1. Verify library name is correct
2. Check Context7 has documentation for that library
3. Try a different search query

## Examples

### API Integration Skill

```markdown
---
name: stripe-api
description: Integration patterns for Stripe payments
tags: [payments, api, stripe]
version: 1.0.0
---

# Stripe API Integration

## Setup

```bash
bun add stripe
```

## Creating Charges

```typescript
import Stripe from "stripe";

const stripe = new Stripe(process.env.STRIPE_SECRET_KEY);

const charge = await stripe.charges.create({
  amount: 2000,  // $20.00 in cents
  currency: "usd",
  source: tokenId,
  description: "My Product",
});
```

## Webhooks

```typescript
app.post("/webhook", (req, res) => {
  const event = stripe.webhooks.constructEvent(
    req.body,
    req.headers["stripe-signature"],
    process.env.STRIPE_WEBHOOK_SECRET
  );
  
  switch (event.type) {
    case "payment_intent.succeeded":
      // Handle success
      break;
  }
});
```
```

### Testing Skill

```markdown
---
name: testing-patterns
description: Best practices for testing Rust code
tags: [rust, testing, best-practices]
version: 1.0.0
---

# Rust Testing Patterns

## Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_addition() {
        assert_eq!(add(2, 2), 4);
    }
    
    #[test]
    #[should_panic(expected = "divide by zero")]
    fn test_divide_by_zero() {
        divide(10, 0);
    }
}
```

## Integration Tests

```rust
// tests/integration_test.rs

#[tokio::test]
async fn test_api_endpoint() {
    let app = create_test_app().await;
    let response = app
        .oneshot(Request::builder()
            .uri("/api/users")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}
```
```

## See Also

- [ARCHITECTURE.md](./ARCHITECTURE.md) - MCP Compatibility section
- [UX_PRINCIPLES.md](./UX_PRINCIPLES.md) - Human-in-the-loop and transparency guardrails
- [CODING_STANDARDS.md](./CODING_STANDARDS.md) - Documentation standards
- [Model Context Protocol](https://modelcontextprotocol.io/)
