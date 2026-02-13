---
name: orchestrix-app-development
description: This skill should be used when implementing or reviewing Orchestrix features that touch Tauri/Rust orchestration, event-driven React UI behavior, human-in-the-loop review flow, performance/scalability constraints, or design-system alignment.
---

# Orchestrix App Development

## Overview

Apply Orchestrix-specific architecture, UX, visual system, and delivery constraints while implementing features, fixes, and refactors. Keep backend orchestration authoritative, preserve full event transparency, maintain a condensed non-cluttered timeline UX, and enforce the Orchestrix design system.

## Use This Skill When

Trigger this skill for requests such as:

- Add or modify task lifecycle behavior (plan mode, build mode, review gates).
- Add or modify agent/tool/event flows in `src-tauri/`.
- Update frontend timeline, task panels, artifact review, or event consumption in `src/`.
- Add or refine design tokens, component styling, shell layout, or interaction polish.
- Improve scaling/performance for long runs or high-frequency event streams.
- Align product behavior with `AGENTS.md`, `ARCHITECTURE.md`, `UX_PRINCIPLES.md`, `DESIGN_SYSTEM.md`, and `CODING_STANDARDS.md`.

## Core Execution Rules

Follow these rules for every change:

1. Keep orchestration in Rust backend; keep frontend render-only for state and user intent.
2. Preserve human-in-the-loop checkpoints: plan review, execution visibility, intervention/cancel capability.
3. Ensure no meaningful model/tool transition happens without an auditable event.
4. Keep timeline summary-first with progressive disclosure for deep details.
5. Protect performance by using batching-aware, incremental event handling and narrow store selectors.
6. Preserve crash recovery assumptions: reconstructability from DB + events.
7. Use Bun-only workflows for package management and scripts.
8. For UI work, consume design tokens and component standards from `DESIGN_SYSTEM.md` and avoid one-off styling.
9. Reject generic UI patterns: each screen must have a focal area, contrast ladder, and intentional composition.
10. Enforce a "don't make me think" bar: familiar affordances, obvious next action, minimal decision friction.

## Workflow

### 1) Map Request to Contracts

Read and align against:

- `AGENTS.md`
- `UX_PRINCIPLES.md`
- `DESIGN_SYSTEM.md`
- `ARCHITECTURE.md`
- `CODING_STANDARDS.md`

Use `references/doc-map.md` to quickly find target sections.

For UI changes, treat `DESIGN_SYSTEM.md` as normative for tokens, spacing/depth, component behavior, and motion/accessibility constraints.

### 2) Identify Impact Surface

Classify changes as one or more of:

- Runtime orchestration (`src-tauri/src/runtime/`, `src-tauri/src/commands/`)
- Event bus/contract (`src-tauri/src/bus/`, event payload/type updates)
- Store/event consumption (`src/store.ts`, `src/stores/`, `src/runtime/`)
- Timeline UX (`src/components/Chat/`, artifacts/review surfaces)
- Design system and theming (`src/index.css`, `src/components/ui/`, shell/layout surfaces)
- Docs/spec alignment (root markdown docs)

### 3) Implement with Transparency and UX Discipline

- Emit and persist events for meaningful transitions.
- Preserve immediate feedback events (`agent.deciding`, `agent.tool_calls_preparing`) where relevant.
- Keep summary content concise; move verbose payloads behind expansion.
- Avoid duplicate or conflicting status surfaces.
- Keep visual hierarchy calm and professional; prioritize readability over decorative effects.

### 3b) Apply Design-System Guardrails for UI Work

- Use semantic tokens (`primary`, `muted`, `accent`, `success`, `warning`, `info`, `destructive`) instead of hardcoded color literals.
- Preserve light/dark parity when introducing new surfaces or states.
- Reuse existing primitives in `src/components/ui/` before creating custom variants.
- Keep timeline and review surfaces condensed-first; details must be opt-in.
- Ensure all interactive controls have visible focus rings and keyboard-accessible flows.
- Use restrained motion (roughly 120-280ms) only to communicate state change.

### 3c) Apply Distinctive UI Craft Rules (Required)

- Build a clear **focal area** per screen (what users should notice first in the initial viewport).
- Enforce a visible **contrast ladder**: primary action/context, secondary support, passive metadata.
- Group controls by **intent** (create, inspect, approve/cancel), not only by component type.
- Use one atmospheric treatment per region (subtle gradient/texture), never stacked decorative effects.
- Keep semantic color concentrated on status and critical actions; preserve neutral text dominance.
- Prefer concise, stateful microcopy (`Waiting for review`, `Executing tools`) over vague labels.

### 3d) Apply Simplicity + Flow Rules (Required)

- Remove non-essential controls before adding new visual treatment.
- Keep one preferred action clearly dominant in each decision step.
- Follow expected conventions (button, link, icon meanings) unless there is a measurable usability gain.
- For complex paths, map a shortest-click flow and provide direct-jump affordances (search/filter/quick actions).
- Use spacing and typography hierarchy as the primary clarity tools; use animation/depth as secondary reinforcement.

### 4) Validate Scalability and Correctness

Verify:

- Event batching assumptions remain valid.
- Long-run rendering remains responsive (incremental transform + list strategy).
- State remains reconstructible after restart.
- Human approval and intervention points are not bypassed.
- UI changes remain consistent with `DESIGN_SYSTEM.md` quality checklist.
- First-time users can complete the primary task without asking what to click next.

### 5) Report Changes in Product Terms

Communicate outcomes in terms of:

- User involvement
- Transparency
- Condensed readability
- Performance/scalability

## Practical Checklists

### Backend Checklist

- Keep tool permissions and policy boundaries intact.
- Add/adjust events for new transitions.
- Preserve append-only event behavior.
- Keep run/task/sub-agent transitions explicit and auditable.

### Frontend Checklist

- Use selector-based state access; avoid broad subscriptions.
- Process event batches incrementally.
- Keep default timeline rows condensed.
- Expose detailed payloads on demand.
- Keep primary affordances conventional and instantly recognizable.

### Design System Checklist

- Use existing tokens in `src/index.css`; do not introduce arbitrary palette values.
- Maintain established depth model (`elevation-1/2/3`) and radius scale.
- Keep one clear primary action per surface.
- Preserve neutral text hierarchy; keep semantic color concentrated on status cues and key actions.
- Verify dark mode readability and focus visibility for all new interactions.
- Confirm first-viewport composition is not dashboard boilerplate (equal-weight cards, repeated shells).
- Confirm every major surface has a dominant signal and supporting hierarchy.
- Confirm spacing rhythm was tuned from generous to compact, not the reverse.
- Confirm competing actions have intentional emphasis differences.

### UX Checklist

- Keep user aware of current phase (`planning`, `awaiting_review`, `executing`, `completed`, `failed`).
- Keep review/approve/cancel flows visible and clear.
- Elevate warnings/errors above routine logs.
- Validate shortest-path flow for at least one first-time-user task per major UI change.
- Capture and fix hesitation moments (pause/confusion before primary action).

### UI Anti-Patterns (Block on Sight)

- Equal visual weight across all panels and cards
- Repetitive card grids for unrelated information types
- Multi-effect decoration stacks (glow + blur + gradients) that reduce readability
- Ambiguous action labels that hide user outcomes
- Semantic color used as generic decoration instead of status/action meaning
- Novel affordances that break learned UI expectations without a tested benefit
- Dense control clusters that require users to inspect every option before acting

## References

- `references/doc-map.md` - Fast doc lookup for architecture and UX contracts
- `references/change-checklist.md` - Pre-merge validation checklist for Orchestrix changes
- `DESIGN_SYSTEM.md` - Visual tokens, component standards, and UI quality guardrails
