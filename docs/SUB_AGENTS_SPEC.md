# Sub-Agent Execution Spec

This document defines how sub-agents are delegated, executed, joined, and closed.

The goals are:
- deterministic execution,
- strict parent-child boundaries,
- explicit lifecycle transitions,
- structured outputs and gating,
- human-visible traceability for every delegated action.

## 1) Scope

Sub-agents are execution units spawned by a parent run to complete planner-defined work.

Sub-agents are not free-form background workers. They are contract-bound delegates with:
- bounded context,
- bounded permissions,
- bounded runtime,
- explicit closure.

## 2) Delegation Contract

Each sub-agent MUST be created with a delegation contract persisted in `sub_agents.context_json`.

Minimum contract fields:

```json
{
  "parent": {
    "run_id": "<run-id>",
    "step_idx": 0,
    "task_prompt": "<original task prompt>",
    "goal_summary": "<plan goal summary>"
  },
  "step": {
    "title": "<step title>",
    "description": "<step description>",
    "success_criteria": ["<criterion>"]
  },
  "permissions": {
    "allowed_tools": ["<tool-name>"],
    "can_spawn_children": false,
    "max_delegation_depth": 0
  },
  "execution": {
    "attempt_timeout_ms": 90000,
    "max_retries": 1,
    "close_on_completion": true
  },
  "outputs": {
    "report_format": "markdown",
    "report_path_pattern": ".orchestrix/step-<idx>-result.md"
  }
}
```

Rules:
- Parent context is immutable from the child perspective.
- Child can only use tools in `permissions.allowed_tools`.
- Child delegation defaults to disabled (`can_spawn_children=false`).
- If child delegation is enabled, it MUST be depth-limited and explicitly auditable.

## 3) Lifecycle

All sub-agents use this state machine:

`created -> running -> waiting_for_merge -> completed|failed -> closed`

Definitions:
- `created`: row inserted and contract persisted.
- `running`: worker started in isolated execution context.
- `waiting_for_merge`: worker completed step execution and produced output; parent still must integrate.
- `completed`: parent integration checks passed.
- `failed`: execution, policy, timeout, or integration failure.
- `closed`: terminal cleanup done; no further child activity.

Rules:
- Parent MUST explicitly close each sub-agent (success or failure).
- A child is not considered done until it reaches `closed`.

## 4) Parent Responsibilities

Parent runtime is responsible for:
- spawning with contract,
- monitoring attempts and timeouts,
- joining/integrating child output,
- deciding success/failure by integration outcome,
- closing child and persisting terminal state.

Parent is the only component allowed to mutate global run outcome.

## 5) Join and Gating Rules

- Worker-level completion is not final completion.
- A sub-agent may finish execution but fail integration (for example merge conflict).
- Run completion is gated by post-join child states:
  - success path: all children `closed` with successful integration,
  - failure path: any child `closed` after `failed`.

## 6) Event Contract

Sub-agent events must be append-only and auditable.

They must also remain UI-friendly: summary information should be available for condensed timeline rendering, with full details available on expansion.

Required event sequence per child:
- `agent.subagent_created`
- `agent.subagent_started`
- zero or more `agent.subagent_attempt`
- one of:
  - `agent.subagent_waiting_for_merge`
  - `agent.subagent_failed`
- optional integration events (`agent.worktree_merged`, etc.)
- `agent.subagent_closed`

`agent.subagent_closed` payload must include:
- `sub_agent_id`
- `step_idx`
- `final_status` (`completed` or `failed`)
- `close_reason`

## 7) Determinism and Parallelism

- A single run remains deterministic and parent-authoritative.
- Parallel sub-agent execution is allowed only when planner-defined ownership boundaries do not overlap.
- Integration order remains explicit and reproducible.

## 8) Implementation Notes (Current Integration)

Initial integration in runtime should enforce:
- persisted delegation contract fields in `context_json`,
- tool allowlist checks against contract,
- `delegate` action rejection unless contract explicitly allows it,
- explicit `waiting_for_merge` and `closed` transitions,
- merge failures treated as child failure (not soft warnings).
