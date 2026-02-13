# Orchestrix Documentation Map

Use this map to quickly load the right contract before making changes.

## Primary Contracts

- `AGENTS.md`
  - Core principles
  - Task lifecycle
  - Event rules and immediate vs batched events
  - Frontend contract and non-goals

- `UX_PRINCIPLES.md`
  - Human-in-the-loop checkpoints
  - Transparency contract
  - Condensed visualization rules
  - Scaling/performance guardrails

- `ARCHITECTURE.md`
  - Data flow and event flow
  - Event categories and batching
  - IPC and state flow details

- `CODING_STANDARDS.md`
  - Performance requirements for event handling
  - UX standards for timeline and review flow
  - Event naming/rules and batching expectations

## Supporting Contracts

- `SUB_AGENTS_SPEC.md`
  - Sub-agent lifecycle and closure semantics
  - Event sequence requirements for child runs

- `README.md`
  - Product-level positioning and principle summary

- `SETUP.md`
  - Manual verification flow, including human-in-the-loop UX checks

- `TROUBLESHOOTING.md`
  - UX transparency and timeline-noise diagnostics

## Search Shortcuts

Use these repo searches when context is missing:

- `agent.deciding|agent.tool_calls_preparing`
- `tool.call_started|tool.call_finished`
- `awaiting_review|approve_plan|submit_plan_feedback`
- `eventBuffer|processEvents|conversationItems`
- `useShallow|streamStore|batcher`
