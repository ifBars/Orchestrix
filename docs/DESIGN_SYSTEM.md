# Orchestrix Design System

A professional, modern, minimal design language for the Orchestrix desktop app.

This system is inspired by:
- The structure and implementation clarity of `Fluxel/docs/design_system.md`
- The premium feel of Codalyn's visual style (glass surfaces, atmospheric depth, disciplined contrast), applied with a more restrained, productivity-first tone

---

## 1. Design Intent

### Product Character
- **Professional**: calm, dependable, and precise
- **Modern**: contemporary typography, subtle depth, clear interaction feedback
- **Minimal**: low visual noise, strong hierarchy, no ornamental clutter

### UX North Star
Orchestrix should feel like an operations console for AI work:
- **Readable under load** (long timelines, high event volume)
- **Trustworthy by default** (clear state transitions and transparent status)
- **Fast to navigate** (keyboard-first actions, predictable placement)

### Core Principles
- **Signal over decoration**: every visual layer must convey meaning
- **Hierarchy first**: one obvious primary action per surface
- **State clarity**: planning, review, executing, completed, failed are visually distinct
- **Density with breathing room**: compact layout with enough spacing for scannability
- **Theme parity**: light and dark modes must feel equally polished
- **Distinctive restraint**: avoid generic templates by giving each major surface a clear visual point of view without adding clutter
- **Do not make users think**: interfaces must follow familiar patterns and make next actions obvious at a glance

---

## 2. Visual Direction

### Aesthetic Summary
- Neutral graphite/slate base
- Cool blue signal accent
- Clean rounded geometry
- Soft atmospheric backgrounds (very low contrast)

### Signature Visual DNA
- Surfaces should look intentionally composed, not utility-default:
  - One anchor element per screen (status rail, highlighted action cluster, or contextual panel heading)
  - One supporting texture layer (subtle gradient/noise/pattern), never multiple competing effects
  - One contrast jump for emphasis (headline, phase badge, or critical action)
- Favor asymmetry with structure: balanced columns and rails are preferred over perfectly centered generic cards.
- Prioritize confident whitespace around important content blocks; do not fill all available space with controls.

### Codalyn-Inspired Elements (Used Sparingly)
- **Soft spotlight gradients** behind major surfaces
- **Subtle translucency** on floating panels (`bg-* / 80-90` + blur)
- **Layered shadows** for depth hierarchy

### Guardrails
- Avoid neon saturation and multi-color glow stacks
- Avoid heavy blur that reduces text contrast
- Avoid visual motifs that compete with timeline content
- Avoid interchangeable "dashboard boilerplate" layouts where every panel has identical weight
- Avoid decorative iconography that does not map to task state or action semantics

---

## 3. Theming Architecture

Orchestrix uses a token-first architecture based on CSS custom properties and Tailwind v4 theme mapping.

### Source of Truth
- Core tokens are defined in `src/index.css`
- Tailwind aliases are mapped via `@theme inline`
- Dark mode is enabled through the `.dark` class variant

### Token Categories
- **Foundation**: background, foreground, border, input, ring
- **Surface**: card, popover, sidebar
- **Action**: primary, secondary, accent
- **Semantic**: success, warning, info, destructive
- **Depth**: `--shadow-1`, `--shadow-2`, `--shadow-3`
- **Shape**: `--radius`

### Token Authoring Rules
- Define spacing and typography in `rem`-friendly scales to preserve accessibility and cross-density consistency.
- Prefer global variables over local one-off values, including gradients and shadow presets.
- Any new visual effect (gradient, shadow, hover treatment) must be reusable by at least two surfaces before promotion.

---

## 4. Color System (OKLCH)

### Core Tokens

| Token | Light | Dark | Purpose |
|---|---|---|---|
| `--background` | `oklch(0.948 0.004 240)` | `oklch(0.14 0.01 260)` | App canvas |
| `--foreground` | `oklch(0.145 0.004 255)` | `oklch(0.93 0.006 255)` | Primary text |
| `--card` | `oklch(0.999 0.001 240)` | `oklch(0.18 0.012 260)` | Elevated surface |
| `--popover` | `oklch(0.999 0.001 240)` | `oklch(0.18 0.012 260)` | Floating overlays |
| `--border` | `oklch(0.82 0.006 250)` | `oklch(0.28 0.008 260)` | Dividers, boundaries |
| `--input` | `oklch(0.82 0.006 250)` | `oklch(0.28 0.008 260)` | Form control borders |
| `--ring` | `oklch(0.42 0.018 250)` | `oklch(0.68 0.12 235)` | Focus indication |

### Interaction Tokens

| Token | Light | Dark | Purpose |
|---|---|---|---|
| `--primary` | `oklch(0.3 0.02 252)` | `oklch(0.68 0.14 235)` | Primary action |
| `--primary-foreground` | `oklch(0.99 0 0)` | `oklch(0.14 0.01 260)` | Text on primary |
| `--secondary` | `oklch(0.93 0.004 242)` | `oklch(0.24 0.01 260)` | Secondary surfaces |
| `--muted` | `oklch(0.912 0.004 242)` | `oklch(0.22 0.01 260)` | Quiet backgrounds |
| `--muted-foreground` | `oklch(0.37 0.008 255)` | `oklch(0.65 0.008 260)` | Secondary text |
| `--accent` | `oklch(0.905 0.006 240)` | `oklch(0.26 0.018 240)` | Hover/selection surfaces |

### Semantic Tokens

| Token | Light | Dark | Usage |
|---|---|---|---|
| `--success` | `oklch(0.58 0.13 153)` | `oklch(0.72 0.15 155)` | Completed states |
| `--warning` | `oklch(0.68 0.11 85)` | `oklch(0.79 0.13 85)` | Review/attention |
| `--info` | `oklch(0.5 0.055 245)` | `oklch(0.72 0.12 240)` | Active/in-progress |
| `--destructive` | `oklch(0.56 0.2 25)` | `oklch(0.64 0.2 24)` | Failures/danger |

### Sidebar Tokens

| Token | Light | Dark | Purpose |
|---|---|---|---|
| `--sidebar` | `oklch(0.94 0.004 242)` | `oklch(0.12 0.008 260)` | Sidebar background |
| `--sidebar-foreground` | `oklch(0.15 0.004 255)` | `oklch(0.93 0.006 255)` | Sidebar text |
| `--sidebar-border` | `oklch(0.81 0.006 250)` | `oklch(0.24 0.008 260)` | Sidebar separators |

---

## 5. Typography

Typography should feel editorial but restrained.

### Font Stack
- `--font-sans`: `"Geist", "IBM Plex Sans", "Segoe UI", sans-serif`
- `--font-mono`: `"JetBrains Mono", "Cascadia Code", monospace`

### Typographic Character Rules
- Headlines should carry intent: concise wording, tighter tracking (`tracking-tight`), and medium/semibold weight.
- Body copy stays neutral and compact: avoid oversized paragraph text in operational surfaces.
- Metadata must remain visibly secondary via size and contrast, not by reducing readability.
- Reserve mono text for technical payloads (tool names, event IDs, code paths, timestamps).

### Type Scale

| Token | Size | Use |
|---|---|---|
| `text-xs` | `0.75rem` | metadata, timestamps |
| `text-sm` | `0.875rem` | default UI text |
| `text-base` | `1rem` | long-form copy |
| `text-lg` | `1.125rem` | section headers |
| `text-xl` | `1.25rem` | page titles |

### Weight and Rhythm
- Default body: `font-normal`
- Action labels and key metadata: `font-medium` or `font-semibold`
- Prefer tighter headings (`tracking-tight`) and normal body spacing

### Contrast Ladder (Must Be Observable)
Every major screen should exhibit a clear three-step emphasis ladder:

1. **Primary signal**: active phase, primary action, or current task context
2. **Secondary signal**: supporting actions and nearby context
3. **Background signal**: passive metadata and historical detail

If these layers are visually indistinguishable, the screen is considered under-designed.

---

## 6. Spacing, Shape, and Depth

### Spacing Scale (4px base)
- `1` = 4px
- `2` = 8px
- `3` = 12px
- `4` = 16px
- `6` = 24px
- `8` = 32px

### Spacing Method
- Start with generous spacing and reduce until clarity starts to degrade; do not start from compact and expand later.
- Keep spacing increments aligned to the 4px rhythm to avoid inconsistent visual cadence.
- Increase spacing around task-critical controls before increasing color/decorative emphasis.

### Radius
- Base radius token: `--radius: 0.75rem`
- Derived radii:
  - `--radius-sm`: compact controls
  - `--radius-md`: default interactive controls
  - `--radius-lg`: cards and major panels

### Elevation

| Token | Usage |
|---|---|
| `--shadow-1` | sticky title bars, thin separators |
| `--shadow-2` | sidebars and drawer surfaces |
| `--shadow-3` | modal/sheet priority layers |

Use depth to communicate interaction priority, not decoration.

---

## 7. Layout System

### Canonical App Frame
- **Title bar**: 40px height, drag region, compact controls
- **Sidebar**: fixed width (256px), conversation history + global actions
- **Main pane**: timeline/review surface with stable reading column
- **Composer rail**: pinned bottom for active conversation
- **Artifacts panel**: contextual right rail (320px) or overlay on smaller widths

### Spatial Priorities
1. Task status and progress context
2. Current conversation content
3. Tool/technical detail (collapsed by default)
4. Secondary utility controls

### Responsive Rules
- Maintain one clear primary pane at every width
- Collapse secondary surfaces before shrinking core content
- Preserve minimum readable line length in timeline and review panes
- Keep the shortest user path visible on first load; advanced controls can be progressive disclosure
- Provide direct-jump affordances (search/filter/quick actions) to reduce repeated navigation clicks

---

## 8. Component Standards

### Core Primitives (`src/components/ui`)
- `Button`: `default`, `secondary`, `ghost`, `outline`, `destructive`
- `Input`, `Select`, `Textarea`: consistent 36px control height (textarea excepted)
- Focus visible ring required on all interactive fields

### Shell Components
- `Header`: workspace state, theme toggle, window controls
- `Sidebar`: conversation index + provider/skills/agent access
- `IdeShell`: deterministic frame with optional artifact rail

### Composition Rules for Non-Boring Screens
- Every high-level surface must expose a dominant focal area within the first viewport.
- Action groups should be chunked by intent (create, inspect, approve/cancel), not purely by component type.
- Avoid repeating identical card shells for unrelated content types; vary weight by importance.
- Keep decorative treatments local to structural regions (header, rail, modal), not every component.

### Timeline Components
- Event rows should support two densities:
  - **Collapsed**: one-line summary
  - **Expanded**: structured technical details
- Semantic color is applied to status indicators only; body text remains neutral
- Rows should be scannable within seconds: phase, actor/tool, and outcome visible before payload detail

### Review Components
- Markdown preview remains typographically calm and highly legible
- Comment anchors must be visible without competing with content
- Build/approve actions use clear prominence hierarchy
- Competing actions (approve/cancel/retry) must not share equal visual weight

---

## 9. Motion and Feedback

Motion should reinforce state changes and reduce cognitive load.

### Motion Tokens
- Fast: `120ms`
- Default: `180-220ms`
- Slow: `280ms` max
- Easing: `cubic-bezier(0.2, 0.8, 0.2, 1)`

### Recommended Transitions
- Hover and focus: opacity/color only
- Panel open/close: slight translate + fade
- Timeline insertions: fade-up with subtle offset

### Motion Personality
- Motion should feel deliberate and operational, never playful by default.
- Sequence transitions by importance: primary context updates first, secondary details next.
- For dense lists/timelines, prefer quick opacity and position shifts over scale animations.
- Remove animations that do not improve comprehension, feedback, or orientation.

### Loading and Progress
- Use text-first status indicators (`Thinking...`, `Preparing...`)
- Reserve pulsing animation for active states only (`planning`, `executing`)
- Respect reduced-motion preferences

---

## 10. Accessibility and Content

### Accessibility Baseline
- WCAG AA contrast minimum for all text and controls
- Full keyboard navigation for tasking, review, and approvals
- Focus states always visible and non-ambiguous
- Icon-only controls require tooltips/labels
- Conventional affordances stay conventional (buttons look clickable, links look like links, icons use expected meanings)

### Content Style
- Use direct, operational language
- Prefer short, specific labels over conversational filler
- Keep destructive actions explicit (`Delete conversation`, `Cancel run`)

### Microcopy Tone for Professional Clarity
- Name intent before mechanism (example: `Approve Plan` before `Write Artifact`).
- Prefer stateful phrasing that reduces ambiguity (`Waiting for review`, `Executing tools`, `Run cancelled`).
- Avoid vague labels like `Manage`, `Optimize`, or `Handle` without object context.

---

## 11. Atmospheric Layer Recipe

To keep a premium feel without visual noise, use this stack:

1. Base gradient background (already in `src/index.css`)
2. Optional soft spotlight radial layer (very low alpha)
3. Optional panel translucency for overlays (`bg-card/80` + `backdrop-blur-sm`)

Do not combine more than two animated ambient layers in the same viewport.

---

## 12. Implementation Contract

### Required Rules
- New UI surfaces must consume design tokens (no hardcoded hex/HSL values)
- Interactive components must include focus-visible states
- Any new semantic state must map to a token (`success`, `warning`, `info`, `destructive`)
- Timeline views must default to condensed mode with optional expansion
- New screens must demonstrate a clear emphasis ladder (primary, secondary, background)
- New layouts must define a focal area and avoid equal visual weight across all panels
- High-friction user goals must have a mapped shortest-path flow before implementation

### Primary Reference Files
- `src/index.css`
- `src/layouts/IdeShell.tsx`
- `src/components/ui/button.tsx`
- `src/components/ui/input.tsx`
- `src/components/ui/select.tsx`
- `src/components/ui/textarea.tsx`
- `src/components/Sidebar.tsx`
- `src/components/Header.tsx`

---

## 13. Quality Checklist for New UI Work

- Uses existing tokens and semantic colors correctly
- Preserves hierarchy in light and dark mode
- Keeps primary task flow visible without scrolling detours
- Supports keyboard operation and visible focus
- Adds motion only when it communicates state change
- Maintains the professional, minimal Orchestrix tone
- Has a clear focal area and non-generic composition in the first viewport
- Shows an observable contrast ladder between primary actions, support context, and passive metadata
- Uses microcopy that is specific, operational, and state-aware
- Preserves familiar affordances so first-time users can act without guessing
- Keeps key decisions obvious (single preferred action, de-emphasized alternatives)
- Uses spacing and hierarchy first, decorative effects second

---

## 15. References

### Documentation
- [AGENTS.md](./AGENTS.md) - Agent architecture and execution model
- [ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture and data flow
- [UX_PRINCIPLES.md](./UX_PRINCIPLES.md) - UX, transparency, and performance guardrails
- [SETUP.md](./SETUP.md) - Development environment setup
- [CODING_STANDARDS.md](./CODING_STANDARDS.md) - Code conventions and standards

### Skills
- **orchestrix-app-development** - Use when implementing Orchestrix UI features (see `.agents/skills/orchestrix-app-development/SKILL.md`)

---

## 16. Validation Loop (Required for Significant UI Changes)

For major navigation, review, or conversion-critical flows, validate with a task-based loop:

1. Define one target task and its ideal shortest path.
2. Run at least one first-time-user walkthrough (internal or external).
3. Compare against a known strong baseline experience where practical.
4. Capture hesitation points (where users pause or ask "what now?").
5. Revise hierarchy/copy/layout and retest.

If users hesitate on primary actions, the design is not ready.
