import type {
  AgentMessageStream,
  AgentTodoList,
  ConversationItem,
  PlanData,
} from "@/runtime/eventBuffer";
import type {
  ArtifactContentView,
  ArtifactRow,
  BusEvent,
  TaskContextSnapshotView,
  TaskRow,
} from "@/types";
import type { ReviewComment } from "@/hooks/useArtifactReview";

export type LandingPreviewScenarioId = "planning" | "awaiting_review" | "executing";

type ExecutionSummaryView = {
  totalSteps: number;
  completedSteps: number;
  failedSteps: number;
  runningStep: number | null;
  runningTool: string | null;
};

export type LandingPreviewScenario = {
  id: LandingPreviewScenarioId;
  label: string;
  task: TaskRow;
  defaultTab: "chat" | "review";
  plan: PlanData | null;
  assistantMessage: string | null;
  planStream: string | null;
  visibleItems: ConversationItem[];
  activeAgentStream: AgentMessageStream | null;
  rawEvents: BusEvent[];
  agentTodos: AgentTodoList[];
  contextSnapshot: TaskContextSnapshotView | null;
  executionSummary: ExecutionSummaryView | null;
  artifacts: ArtifactRow[];
  markdownArtifacts: ArtifactRow[];
  artifactContentsByPath: Record<string, ArtifactContentView>;
  initialSelectedArtifactPath: string | null;
  initialRailArtifactPath: string | null;
  initialCommentsByArtifact: Record<string, ReviewComment[]>;
};

const now = new Date("2026-03-05T18:30:00.000Z");

function isoMinutesAgo(minutes: number) {
  return new Date(now.getTime() - minutes * 60_000).toISOString();
}

function task(id: string, prompt: string, status: TaskRow["status"], minutesAgo: number): TaskRow {
  const stamp = isoMinutesAgo(minutesAgo);
  return {
    id,
    prompt,
    parent_task_id: null,
    status,
    created_at: stamp,
    updated_at: stamp,
    workspace_root: "C:/Users/ghost/Desktop/Coding/Rust/Tauri/Orchestrix",
  };
}

const planningTask = task(
  "preview-task-planning",
  "Redesign the landing page to feel premium and IDE-native.",
  "planning",
  6
);

const reviewTask = task(
  "preview-task-review",
  "Extract landing preview shell adapters and review the implementation plan.",
  "awaiting_review",
  12
);

const executingTask = task(
  "preview-task-executing",
  "Wire the preview shell to fixture data and keep actions local-only.",
  "executing",
  2
);

export const LANDING_PREVIEW_TASKS: TaskRow[] = [reviewTask, executingTask, planningTask];

const sharedPlan: PlanData = {
  goalSummary:
    "Ship a premium IDE-like landing page that reuses real Orchestrix shell components in a safe preview mode.",
  completionCriteria:
    "Hero, preview, workflow, and proof sections align to the dark app-adjacent visual system and build without Tauri dependencies.",
  steps: [
    {
      title: "Extract browser-safe shell chrome",
      description:
        "Create presentational header, sidebar, and artifact rail views for landing-only preview state.",
    },
    {
      title: "Replace bespoke landing mockups",
      description:
        "Use IdeShell with real timeline and review surfaces instead of standalone terminal illustrations.",
    },
    {
      title: "Introduce fixture-driven preview scenarios",
      description:
        "Model planning, review, and execution states with local task, artifact, and event data.",
    },
    {
      title: "Recompose the page around the product window",
      description:
        "Build a bold hero, proof strip, preview section, workflow explanation, and compressed technical band.",
    },
  ],
};

const reviewPlanPath = "artifacts/landing-plan.md";
const reviewChecklistPath = "artifacts/review-checklist.md";
const patchPath = "artifacts/preview-shell.patch";
const runLogPath = "artifacts/run-log.txt";

const reviewPlanMarkdown = `# Premium IDE-Like Landing Page

## Goal
Make the Orchestrix marketing site feel like a serious developer tool, not a generic SaaS page.

## Implementation
1. Replace custom landing mockups with an "IdeShell" preview.
2. Add browser-safe header, sidebar, and artifact rail adapters.
3. Default the preview to "awaiting_review" and disable all non-local actions.
4. Recompose the page around hero -> proof strip -> preview -> workflow -> proof band.

## Acceptance
- Landing bundle builds without Tauri calls.
- Preview uses fixture data only.
- Visual language stays close to the desktop app.
`;

const reviewChecklistMarkdown = `# Review Checklist

- Hero uses one atmospheric layer only.
- Preview mode is clearly labeled.
- Sidebar, timeline, and review surfaces are interactive.
- No live AI, no tool execution, no file writes.
- Technical proof stays concise and product-led.
`;

const patchPreview = `diff --git a/src/LandingPage.tsx b/src/LandingPage.tsx
index a1b2c3d..e4f5g6h 100644
--- a/src/LandingPage.tsx
+++ b/src/LandingPage.tsx
@@
-        <ExecutionModelSection />
-        <ArchitectureAndVisibilitySection />
-        <AgentsAndToolsSection />
-        <ProvidersAndRecoverySection />
+        <ProofStripSection />
+        <PreviewSection />
+        <WorkflowSection />
+        <TechnicalProofSection />
`;

const runLogPreview = `18:29:02 agent.deciding        turn=1 mode=build
18:29:03 tool.call_started     fs.write src/components/landing/PreviewSection.tsx
18:29:04 tool.call_finished    bytes=2841
18:29:05 tool.call_started     bun run build:landing
18:29:07 tool.call_finished    exit=0
`;

const artifactRows: ArtifactRow[] = [
  {
    id: "artifact-plan",
    run_id: "preview-run-review",
    kind: "plan",
    uri_or_content: reviewPlanPath,
    metadata_json: null,
    created_at: isoMinutesAgo(10),
  },
  {
    id: "artifact-review-checklist",
    run_id: "preview-run-review",
    kind: "notes",
    uri_or_content: reviewChecklistPath,
    metadata_json: null,
    created_at: isoMinutesAgo(9),
  },
  {
    id: "artifact-patch",
    run_id: "preview-run-executing",
    kind: "patch",
    uri_or_content: patchPath,
    metadata_json: null,
    created_at: isoMinutesAgo(4),
  },
  {
    id: "artifact-log",
    run_id: "preview-run-executing",
    kind: "log",
    uri_or_content: runLogPath,
    metadata_json: null,
    created_at: isoMinutesAgo(2),
  },
];

const artifactContentsByPath: Record<string, ArtifactContentView> = {
  [reviewPlanPath]: {
    path: reviewPlanPath,
    content: reviewPlanMarkdown,
    is_markdown: true,
  },
  [reviewChecklistPath]: {
    path: reviewChecklistPath,
    content: reviewChecklistMarkdown,
    is_markdown: true,
  },
  [patchPath]: {
    path: patchPath,
    content: patchPreview,
    is_markdown: false,
  },
  [runLogPath]: {
    path: runLogPath,
    content: runLogPreview,
    is_markdown: false,
  },
};

const reviewComments: Record<string, ReviewComment[]> = {
  [reviewPlanPath]: [
    {
      id: "comment-1",
      line: 8,
      text: "Call out that the preview uses real timeline and review components rather than static screenshots.",
    },
    {
      id: "comment-2",
      line: 13,
      text: "Keep the acceptance criteria tied to browser-only rendering and disabled live actions.",
    },
  ],
};

function toolCall(
  id: string,
  seq: number,
  name: string,
  status: "running" | "success" | "error",
  result: string,
  rationale: string,
  args?: Record<string, unknown>
): ConversationItem {
  return {
    id,
    type: "toolCall",
    timestamp: isoMinutesAgo(1),
    seq,
    toolName: name,
    toolStatus: status,
    toolResult: result,
    toolRationale: rationale,
    toolArgs: args,
  };
}

const planningScenario: LandingPreviewScenario = {
  id: "planning",
  label: "Planning",
  task: planningTask,
  defaultTab: "chat",
  plan: sharedPlan,
  assistantMessage:
    "I inspected the landing entrypoint and the desktop shell. The cleanest implementation path is to reuse IdeShell, ConversationTimeline, and ReviewWorkspace, then feed them browser-safe fixture data.",
  planStream: null,
  visibleItems: [
    {
      id: "planning-status",
      type: "statusChange",
      timestamp: isoMinutesAgo(6),
      seq: 1,
      status: "deciding",
      content: "Exploring landing entrypoints and reusable desktop surfaces",
    },
    toolCall(
      "planning-tool-1",
      2,
      "fs.list",
      "success",
      "Found isolated landing entrypoint, CSS, and section components.",
      "Map the existing landing surface before rewriting it.",
      { path: "src/components/landing" }
    ),
    toolCall(
      "planning-tool-2",
      3,
      "fs.read",
      "success",
      "Confirmed IdeShell, ConversationTimeline, and ReviewWorkspace are browser-safe enough to reuse with local props.",
      "Check which app surfaces can be safely reused in the landing preview.",
      { path: "src/layouts/IdeShell.tsx" }
    ),
    {
      id: "planning-thinking",
      type: "thinking",
      timestamp: isoMinutesAgo(5),
      seq: 4,
      content:
        "The landing shell should not mount live app state. Extracting a small set of presentational chrome views is lower risk than trying to hydrate the full app in browser-only mode.",
    },
    {
      id: "planning-message",
      type: "agentMessage",
      timestamp: isoMinutesAgo(4),
      seq: 5,
      content:
        "Next I will swap the page structure to hero -> proof strip -> preview -> workflow -> technical proof, then connect the preview to local scenarios for planning, review, and execution.",
    },
  ],
  activeAgentStream: null,
  rawEvents: [],
  agentTodos: [],
  contextSnapshot: null,
  executionSummary: null,
  artifacts: [artifactRows[0], artifactRows[1]],
  markdownArtifacts: [artifactRows[0], artifactRows[1]],
  artifactContentsByPath,
  initialSelectedArtifactPath: reviewPlanPath,
  initialRailArtifactPath: reviewChecklistPath,
  initialCommentsByArtifact: reviewComments,
};

const reviewScenario: LandingPreviewScenario = {
  id: "awaiting_review",
  label: "Awaiting Review",
  task: reviewTask,
  defaultTab: "review",
  plan: sharedPlan,
  assistantMessage:
    "I drafted the implementation plan and attached the artifact. The preview defaults to review mode because this is where Orchestrix differentiates itself from most IDE sites.",
  planStream: null,
  visibleItems: [
    {
      id: "review-status",
      type: "statusChange",
      timestamp: isoMinutesAgo(12),
      seq: 1,
      status: "awaiting_review",
      content: "Plan ready - waiting for approval before any implementation work.",
    },
    {
      id: "review-message",
      type: "agentMessage",
      timestamp: isoMinutesAgo(11),
      seq: 2,
      content:
        "I kept the visual direction close to the app: darker chrome, restrained blue accent, larger product window, and an interactive preview powered by fixture data only.",
    },
  ],
  activeAgentStream: null,
  rawEvents: [],
  agentTodos: [],
  contextSnapshot: null,
  executionSummary: null,
  artifacts: [artifactRows[0], artifactRows[1], artifactRows[2]],
  markdownArtifacts: [artifactRows[0], artifactRows[1]],
  artifactContentsByPath,
  initialSelectedArtifactPath: reviewPlanPath,
  initialRailArtifactPath: patchPath,
  initialCommentsByArtifact: reviewComments,
};

const executingScenario: LandingPreviewScenario = {
  id: "executing",
  label: "Executing",
  task: executingTask,
  defaultTab: "chat",
  plan: sharedPlan,
  assistantMessage:
    "The approved plan is now executing against the landing surface with full artifact visibility and a browser-safe preview shell.",
  planStream: null,
  visibleItems: [
    {
      id: "executing-status",
      type: "statusChange",
      timestamp: isoMinutesAgo(2),
      seq: 1,
      status: "executing",
      content: "Executing approved plan - wiring preview shell to fixture data.",
    },
    toolCall(
      "executing-tool-1",
      2,
      "fs.write",
      "success",
      "Created browser-safe header and sidebar adapters for landing preview.",
      "Extract presentational shell chrome without touching app state.",
      { path: "src/components/landing/preview/PreviewChrome.tsx" }
    ),
    toolCall(
      "executing-tool-2",
      3,
      "fs.write",
      "success",
      "Preview scenarios now drive real timeline and review components.",
      "Replace bespoke mockups with fixture-driven product surfaces.",
      { path: "src/components/landing/preview/previewData.ts" }
    ),
    toolCall(
      "executing-tool-3",
      4,
      "bun run build:landing",
      "running",
      "Bundling landing entrypoint and validating browser-safe rendering...",
      "Verify the redesigned landing bundle without Tauri APIs.",
      { cwd: "C:/Users/ghost/Desktop/Coding/Rust/Tauri/Orchestrix" }
    ),
    {
      id: "executing-file-1",
      type: "fileChange",
      timestamp: isoMinutesAgo(1),
      seq: 5,
      filePath: "src/components/landing/PreviewSection.tsx",
      fileAction: "write",
    },
    {
      id: "executing-thinking",
      type: "thinking",
      timestamp: isoMinutesAgo(1),
      seq: 6,
      content:
        "Keep the preview honest: interactive shell controls are fine, but any action that looks like it would execute AI work should remain disabled or explicitly labeled as local-only.",
    },
  ],
  activeAgentStream: {
    streamId: "preview-stream-1",
    content:
      "The preview shell is mounted and the landing sections are swapping over. I am validating that the review surface still behaves correctly when fed edited fixture content instead of app-store artifacts.",
    startedAt: isoMinutesAgo(1),
    updatedAt: isoMinutesAgo(0),
    seq: 7,
    isStreaming: true,
  },
  rawEvents: [],
  agentTodos: [
    {
      agentId: "main",
      updatedAt: isoMinutesAgo(1),
      todos: [
        {
          id: "todo-1",
          content: "Extract browser-safe shell header and sidebar views",
          status: "completed",
          priority: "high",
        },
        {
          id: "todo-2",
          content: "Swap the preview section to use real timeline and review components",
          status: "completed",
          priority: "high",
        },
        {
          id: "todo-3",
          content: "Polish hero chrome and premium landing composition",
          status: "in_progress",
          priority: "medium",
        },
      ],
    },
  ],
  contextSnapshot: {
    task_id: executingTask.id,
    provider: "OpenAI-compatible",
    model: "preview-fixture",
    mode: "build",
    context_window: 200000,
    used_tokens: 41800,
    free_tokens: 158200,
    usage_percentage: 20.9,
    segments: [
      { key: "system_prompt", label: "System prompt", tokens: 3800, percentage: 1.9 },
      { key: "tool_definitions", label: "Tool definitions", tokens: 8900, percentage: 4.5 },
      { key: "messages", label: "Messages", tokens: 21600, percentage: 10.8 },
      { key: "compaction_buffer", label: "Compaction buffer", tokens: 7500, percentage: 3.7 },
      { key: "free_space", label: "Free space", tokens: 158200, percentage: 79.1 },
    ],
    updated_at: isoMinutesAgo(0),
    estimated: false,
  },
  executionSummary: {
    totalSteps: 4,
    completedSteps: 2,
    failedSteps: 0,
    runningStep: 3,
    runningTool: "bun run build:landing",
  },
  artifacts: artifactRows,
  markdownArtifacts: [artifactRows[0], artifactRows[1]],
  artifactContentsByPath,
  initialSelectedArtifactPath: reviewPlanPath,
  initialRailArtifactPath: patchPath,
  initialCommentsByArtifact: reviewComments,
};

export const LANDING_PREVIEW_SCENARIOS: Record<LandingPreviewScenarioId, LandingPreviewScenario> = {
  planning: planningScenario,
  awaiting_review: reviewScenario,
  executing: executingScenario,
};

export function getScenarioForTask(taskId: string): LandingPreviewScenarioId {
  const matched = Object.values(LANDING_PREVIEW_SCENARIOS).find((scenario) => scenario.task.id === taskId);
  return matched ? matched.id : "awaiting_review";
}

