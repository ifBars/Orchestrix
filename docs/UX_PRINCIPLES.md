# UX, Transparency, and Performance Principles

This document defines the product guardrails for Orchestrix UX so every part of the system stays aligned with a scalable, high-performance, human-in-the-loop agent experience.

## Product Outcomes

Orchestrix should always feel:

- **Involving**: The user stays in the loop and can review, approve, redirect, or cancel work.
- **Transparent**: Users can see what the AI is doing, when it is doing it, and why it is doing it.
- **Condensed**: The UI favors signal over noise with progressive disclosure, grouping, and clear visual hierarchy.
- **Fast at scale**: The interface remains responsive as task count, event volume, and run duration grow.

## Human-in-the-Loop Contract

The user is an active collaborator, not an observer.

Required checkpoints:

1. **Before execution**: Plan must be reviewable and explicitly approved.
2. **During execution**: User can monitor progress in real time and interrupt/cancel safely.
3. **Before risky actions**: Permission-gated operations require explicit approval.
4. **After execution**: Artifacts and outcomes are reviewable with traceability to steps and tool calls.

## Transparency Contract

No meaningful AI action should be hidden from the user.

Required visibility:

- Model turn lifecycle (`agent.deciding`, `agent.tool_calls_preparing`)
- Tool invocation start/end (`tool.call_started`, `tool.call_finished`)
- Plan lifecycle (`agent.planning_started`, `agent.plan_ready`, `artifact.created`)
- Execution milestones (`agent.step_*`, `agent.subagent_*`)
- Failures and recoveries (`system.error`, retry/timeout events)

Every visible item should link back to stable identifiers where possible (`task_id`, `run_id`, `step_idx`, `sub_agent_id`, `tool_call_id`).

## Condensed, Non-Cluttered Visualization

Use progressive disclosure by default:

- Show a compact timeline row first (status + one-line summary + timestamp).
- Expand into full details on demand (arguments, outputs, logs, stack traces).
- Group repetitive high-frequency events under step or phase containers.
- Keep current phase prominent (`Planning`, `Awaiting review`, `Executing`, `Completed`, `Failed`).
- Surface warnings/errors first; keep verbose details collapsible.

Design constraints:

- One primary timeline per task run.
- No duplicate status widgets that drift from event truth.
- No hidden background agent activity.
- Preserve readability on desktop and smaller laptop widths.

## Scaling and Performance Guardrails

The system should handle long-running tasks and high event throughput without UI degradation.

Backend guardrails:

- Event delivery is append-only and persisted.
- High-frequency events are batched (100ms flush, 50-item max).
- Immediate flush for interaction-critical events (`task.*`, `agent.step_*`, `agent.deciding`, `agent.tool_calls_preparing`).

Frontend guardrails:

- Use selector-based subscriptions to avoid broad re-renders.
- Use virtualization/windowing for long timelines.
- Do incremental event processing instead of full-array recomputation.
- Keep expensive transforms off the critical render path.

Operational guardrails:

- Crash recovery must restore task/run state from DB + event history.
- UI state must always be reconstructible from persisted events.

## Documentation Alignment Checklist

Any documentation update should preserve these truths:

- Backend-authoritative orchestration
- Event-driven rendering contract
- Human-in-the-loop plan and execution review
- Full visibility into AI decisions and tool activity
- Condensed but explorable UX patterns
- Scalability and performance constraints

## References

### Documentation
- [AGENTS.md](./AGENTS.md) - Agent architecture and execution model
- [ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture and data flow
- [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md) - Visual design tokens and UI standards
- [SETUP.md](./SETUP.md) - Development environment setup
- [CODING_STANDARDS.md](./CODING_STANDARDS.md) - Code conventions and standards

### Skills
- **orchestrix-app-development** - Use when implementing Orchestrix features (see `.agents/skills/orchestrix-app-development/SKILL.md`)

---

See also: `README.md`, `ARCHITECTURE.md`, `AGENTS.md`, `CODING_STANDARDS.md`.
