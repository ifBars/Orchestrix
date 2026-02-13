# Orchestrix Change Checklist

Run this checklist before concluding an implementation.

## Contract Safety

- Backend remains orchestration authority.
- Frontend remains render + user intent only.
- Human-in-the-loop gates remain intact (plan approval, intervention points).

## Event Transparency

- Meaningful transitions emit events.
- New events follow naming and payload conventions.
- Immediate feedback events remain available where needed.
- Event persistence and append-only behavior remain intact.

## UX Quality

- Timeline remains summary-first and non-cluttered.
- Details are expandable without removing transparency.
- Errors/warnings are visually prioritized.
- No duplicate/conflicting status surfaces are introduced.

## Performance and Scale

- Event handling is incremental, not full-history recomputation.
- Store selectors remain narrow and re-render conscious.
- Long lists/timelines have a strategy for scale.
- Batching assumptions (100ms / 50 max + immediate set) are respected.

## Verification

- Plan flow still transitions through `awaiting_review`.
- Build flow remains visible in timeline with tool-level traceability.
- Restart/recovery path remains reconstructible from DB + events.
