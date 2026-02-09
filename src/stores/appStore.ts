/**
 * Main application state store.
 *
 * This Zustand store manages the core application state for the Orchestrix
 * desktop application. It handles:
 *
 * - Task management (CRUD operations)
 * - Event streaming from the backend
 * - Provider configuration
 * - Workspace settings
 * - Skills management
 *
 * The store communicates with the Rust backend via Tauri's invoke() API
 * and receives real-time updates through the event system.
 *
 * @example
 * ```tsx
 * const { tasks, createTask } = useAppStore();
 *
 * // Create a new task
 * await createTask("Create a React component");
 *
 * // Access task list
 * console.log(tasks);
 * ```
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { create } from "zustand";
import { runtimeEventBuffer } from "@/runtime/eventBuffer";
import { useStreamTickStore } from "@/stores/streamStore";
import type {
  ArtifactRow,
  BusEvent,
  EventRow,
  ModelCatalogEntry,
  NewCustomSkill,
  McpServerConfig,
  McpServerInput,
  McpToolEntry,
  ProviderConfigView,
  RunRow,
  SkillCatalogItem,
  TaskLinkRow,
  TaskStatus,
  TaskRow,
  WorkspaceRootView,
  WorkspaceSkill,
} from "@/types";

const TASK_STATUSES: ReadonlySet<TaskStatus> = new Set([
  "pending",
  "planning",
  "awaiting_review",
  "executing",
  "completed",
  "failed",
  "cancelled",
]);

function isTaskStatus(value: string): value is TaskStatus {
  return TASK_STATUSES.has(value as TaskStatus);
}

type CreateTaskOptions = {
  parentTaskId?: string;
  referenceTaskIds?: string[];
  mode?: "plan" | "build";
};

function linkRowsToIds(taskId: string, links: TaskLinkRow[]): string[] {
  const ids = new Set<string>();
  for (const link of links) {
    if (link.source_task_id === taskId) ids.add(link.target_task_id);
    if (link.target_task_id === taskId) ids.add(link.source_task_id);
  }
  return [...ids];
}

type AppStoreState = {
  tasks: TaskRow[];
  selectedTaskId: string | null;
  providerConfigs: ProviderConfigView[];
  modelCatalog: ModelCatalogEntry[];
  selectedProvider: string;
  selectedModel: string;
  workspaceRoot: string;
  skills: SkillCatalogItem[];
  workspaceSkills: WorkspaceSkill[];
  mcpServers: McpServerConfig[];
  mcpTools: McpToolEntry[];
  artifactsByTask: Record<string, ArtifactRow[]>;
  taskLinksByTask: Record<string, string[]>;
  bootstrapped: boolean;

  bootstrap: () => Promise<void>;
  shutdown: () => void;

  createTask: (prompt: string, options?: CreateTaskOptions) => Promise<void>;
  branchTask: (taskId: string) => Promise<void>;
  deleteTask: (taskId: string) => Promise<void>;
  linkTasks: (taskId: string, relatedTaskId: string) => Promise<void>;
  unlinkTasks: (taskId: string, relatedTaskId: string) => Promise<void>;
  startTask: (taskId: string) => Promise<void>;
  approvePlan: (taskId: string) => Promise<void>;
  submitPlanFeedback: (taskId: string, note: string) => Promise<void>;
  cancelTask: (taskId: string) => Promise<void>;
  sendMessageToTask: (taskId: string, message: string) => Promise<void>;
  selectTask: (taskId: string | null) => void;

  setProviderConfig: (
    provider: string,
    apiKey: string,
    defaultModel?: string,
    baseUrl?: string,
  ) => Promise<void>;
  selectProviderModel: (provider: string, model: string) => void;

  setWorkspaceRoot: (workspaceRoot: string) => Promise<void>;
  refreshWorkspaceRoot: () => Promise<void>;
  refreshTaskArtifacts: (taskId: string) => Promise<void>;
  refreshTaskLinks: (taskId: string) => Promise<void>;
  refreshSkills: () => Promise<void>;
  searchSkills: (query: string, source?: string, limit?: number) => Promise<SkillCatalogItem[]>;
  addCustomSkill: (skill: NewCustomSkill) => Promise<void>;
  importContext7Skill: (libraryId: string, title?: string) => Promise<void>;
  importVercelSkill: (skillName: string) => Promise<void>;
  removeSkill: (skillId: string) => Promise<void>;
  refreshWorkspaceSkills: () => Promise<void>;
  getWorkspaceSkillContent: (skillId: string) => Promise<WorkspaceSkill>;
  refreshMcpServers: () => Promise<void>;
  upsertMcpServer: (server: McpServerInput) => Promise<void>;
  removeMcpServer: (serverId: string) => Promise<void>;
  refreshMcpTools: () => Promise<void>;
};

let unlistenEvents: UnlistenFn | null = null;

function modelForProvider(provider: string, configs: ProviderConfigView[], catalog: ModelCatalogEntry[]): string {
  const config = configs.find((value) => value.provider === provider);
  if (config?.default_model) return config.default_model;
  const entry = catalog.find((value) => value.provider === provider);
  return entry?.models[0] ?? "";
}

export const useAppStore = create<AppStoreState>((set, get) => ({
  tasks: [],
  selectedTaskId: null,
  providerConfigs: [],
  modelCatalog: [],
  selectedProvider: "minimax",
  selectedModel: "MiniMax-M2.1",
  workspaceRoot: "",
  skills: [],
  workspaceSkills: [],
  mcpServers: [],
  mcpTools: [],
  artifactsByTask: {},
  taskLinksByTask: {},
  bootstrapped: false,

  bootstrap: async () => {
    if (get().bootstrapped) return;

    const [tasks, providerConfigs, modelCatalog, workspaceRoot, skills, workspaceSkills, mcpServers, mcpTools] = await Promise.all([
      invoke<TaskRow[]>("list_tasks"),
      invoke<ProviderConfigView[]>("get_provider_configs"),
      invoke<ModelCatalogEntry[]>("get_model_catalog"),
      invoke<WorkspaceRootView>("get_workspace_root"),
      invoke<SkillCatalogItem[]>("list_available_skills"),
      invoke<WorkspaceSkill[]>("list_workspace_skills"),
      invoke<McpServerConfig[]>("list_mcp_server_configs"),
      invoke<McpToolEntry[]>("list_cached_mcp_tools"),
    ]);

    const linkResults = await Promise.all(
      tasks.map(async (task) => {
        const links = await invoke<TaskLinkRow[]>("list_task_links", { taskId: task.id });
        return [task.id, linkRowsToIds(task.id, links)] as const;
      })
    );
    const taskLinksByTask = Object.fromEntries(linkResults);

    const artifactResults = await Promise.all(
      tasks.map(async (task) => {
        const latestRun = await invoke<RunRow | null>("get_latest_run", { taskId: task.id });
        if (!latestRun?.id) {
          return [task.id, [] as ArtifactRow[], null] as const;
        }
        const artifacts = await invoke<ArtifactRow[]>("list_run_artifacts", { runId: latestRun.id });
        return [task.id, artifacts, latestRun.status] as const;
      })
    );
    const artifactsByTask = Object.fromEntries(artifactResults.map(([taskId, artifacts]) => [taskId, artifacts]));
    const latestRunStatusByTask = Object.fromEntries(artifactResults.map(([taskId, _artifacts, status]) => [taskId, status]));

    const normalizedTasks = tasks.map((task) => {
      const latestRunStatus = latestRunStatusByTask[task.id] as string | null | undefined;
      const artifacts = artifactsByTask[task.id] ?? [];

      if (task.status === "planning") {
        if (latestRunStatus === "awaiting_review" || artifacts.length > 0) {
          return { ...task, status: "awaiting_review" as TaskStatus };
        }
      }

      return task;
    });

    const selectedProvider = "minimax";
    const selectedModel = modelForProvider(selectedProvider, providerConfigs, modelCatalog);

    set({
      tasks: normalizedTasks,
      providerConfigs,
      modelCatalog,
      selectedProvider,
      selectedModel,
      workspaceRoot: workspaceRoot.workspace_root,
      skills,
      workspaceSkills,
      mcpServers,
      mcpTools,
      artifactsByTask,
      taskLinksByTask,
      bootstrapped: true,
    });

    // Load historical events for all tasks to restore chat history
    const planTaskIds = new Set<string>();
    const timelineTaskIds = new Set<string>();
    
    await Promise.all(
      tasks.map(async (task) => {
        try {
          const eventRows = await invoke<EventRow[]>("get_task_events", { taskId: task.id });
          if (eventRows.length > 0) {
            // Convert EventRow to BusEvent and replay through event buffer
            const busEvents: BusEvent[] = eventRows.map((row) => ({
              id: row.id,
              run_id: row.run_id,
              seq: row.seq,
              category: row.category,
              event_type: row.event_type,
              payload: JSON.parse(row.payload_json),
              created_at: row.created_at,
            }));

            // Replay events into the buffer
            for (const event of busEvents) {
              const result = runtimeEventBuffer.ingest(event, task.id);
              if (result.planChanged) planTaskIds.add(task.id);
              if (result.timelineChanged) timelineTaskIds.add(task.id);
            }
          }
        } catch (e) {
          console.error(`Failed to load events for task ${task.id}:`, e);
        }
      })
    );

    // Trigger UI updates for tasks that had events
    if (planTaskIds.size > 0 || timelineTaskIds.size > 0) {
      useStreamTickStore.getState().bumpBatch(planTaskIds, timelineTaskIds);
    }

    unlistenEvents = await listen<BusEvent[]>("orchestrix://events", (e) => {
      const batch = e.payload;
      if (!Array.isArray(batch) || batch.length === 0) return;

      const planTaskIds = new Set<string>();
      const timelineTaskIds = new Set<string>();
      const taskStatusById = new Map<string, TaskStatus>();
      const deletedTaskIds = new Set<string>();
      const linkedTaskIds = new Set<string>();

      for (const event of batch) {
        const taskId = runtimeEventBuffer.resolveTaskId(event);
        if (!taskId) continue;

        const ingest = runtimeEventBuffer.ingest(event, taskId);
        if (ingest.planChanged) planTaskIds.add(taskId);
        if (ingest.timelineChanged) timelineTaskIds.add(taskId);

        if (event.event_type === "task.status_changed") {
          const status = event.payload?.status;
          if (typeof status === "string" && isTaskStatus(status)) {
            taskStatusById.set(taskId, status);
          }
        }

        if (event.event_type === "task.deleted") {
          deletedTaskIds.add(taskId);
        }

        if (event.event_type === "task.linked" || event.event_type === "task.unlinked") {
          linkedTaskIds.add(taskId);
          const relatedId = event.payload?.related_task_id;
          if (typeof relatedId === "string") linkedTaskIds.add(relatedId);
        }
      }

      if (deletedTaskIds.size > 0) {
        set((state) => {
          const deleted = [...deletedTaskIds];
          const nextTasks = state.tasks.filter((task) => !deletedTaskIds.has(task.id));
          const nextArtifactsByTask = { ...state.artifactsByTask };
          const nextTaskLinksByTask = { ...state.taskLinksByTask };
          for (const taskId of deleted) {
            delete nextArtifactsByTask[taskId];
            delete nextTaskLinksByTask[taskId];
            runtimeEventBuffer.clearTask(taskId);
          }
          for (const [taskId, linkedIds] of Object.entries(nextTaskLinksByTask)) {
            nextTaskLinksByTask[taskId] = linkedIds.filter((linkedId) => !deletedTaskIds.has(linkedId));
          }

          const nextSelected = state.selectedTaskId && deletedTaskIds.has(state.selectedTaskId)
            ? null
            : state.selectedTaskId;

          return {
            tasks: nextTasks,
            selectedTaskId: nextSelected,
            artifactsByTask: nextArtifactsByTask,
            taskLinksByTask: nextTaskLinksByTask,
          };
        });
      }

      if (taskStatusById.size > 0) {
        set((state) => {
          let mutated = false;
          const nextTasks = state.tasks.map((task) => {
            const nextStatus = taskStatusById.get(task.id);
            if (!nextStatus || nextStatus === task.status) return task;
            mutated = true;
            return {
              ...task,
              status: nextStatus,
              updated_at: new Date().toISOString(),
            };
          });
          return mutated ? { tasks: nextTasks } : state;
        });
      }

      const artifactEvents = batch.filter((event) => event.event_type === "artifact.created");
      if (artifactEvents.length > 0) {
        set((state) => {
          const next = { ...state.artifactsByTask };
          let mutated = false;

          for (const event of artifactEvents) {
            const taskId = typeof event.payload?.task_id === "string" ? event.payload.task_id : null;
            const artifactId = typeof event.payload?.artifact_id === "string" ? event.payload.artifact_id : null;
            const kind = typeof event.payload?.kind === "string" ? event.payload.kind : null;
            const uri = typeof event.payload?.uri === "string" ? event.payload.uri : null;
            const runId = event.run_id;
            if (!taskId || !artifactId || !kind || !uri || !runId) continue;

            const prev = next[taskId] ?? [];
            if (prev.some((item) => item.id === artifactId)) continue;

            mutated = true;
            next[taskId] = [
              {
                id: artifactId,
                run_id: runId,
                kind,
                uri_or_content: uri,
                metadata_json: null,
                created_at: event.created_at,
              },
              ...prev,
            ];
          }

          return mutated ? { artifactsByTask: next } : state;
        });
      }

      if (linkedTaskIds.size > 0) {
        Promise.all(
          [...linkedTaskIds].map(async (taskId) => {
            const links = await invoke<TaskLinkRow[]>("list_task_links", { taskId });
            return [taskId, linkRowsToIds(taskId, links)] as const;
          })
        )
          .then((entries) => {
            set((state) => ({
              taskLinksByTask: {
                ...state.taskLinksByTask,
                ...Object.fromEntries(entries),
              },
            }));
          })
          .catch((error) => {
            console.error("Failed to refresh task links", error);
          });
      }

      useStreamTickStore.getState().bumpBatch(planTaskIds, timelineTaskIds);
    });
  },

  shutdown: () => {
    unlistenEvents?.();
    unlistenEvents = null;
    set({ bootstrapped: false });
  },

  createTask: async (prompt: string, options?: CreateTaskOptions) => {
    const created = await invoke<TaskRow>("create_task", {
      prompt,
      options: {
        parent_task_id: options?.parentTaskId ?? null,
        reference_task_ids: options?.referenceTaskIds ?? null,
      },
    });
    const state = get();
    try {
      const command = options?.mode === "build" ? "run_build_mode" : "run_plan_mode";
      await invoke(command, {
        taskId: created.id,
        provider: state.selectedProvider,
        model: state.selectedModel,
      });
    } catch (error) {
      console.error("Auto-run failed", error);
    }
    const [tasks, links] = await Promise.all([
      invoke<TaskRow[]>("list_tasks"),
      invoke<TaskLinkRow[]>("list_task_links", { taskId: created.id }),
    ]);
    set((prev) => ({
      tasks,
      selectedTaskId: created.id,
      taskLinksByTask: {
        ...prev.taskLinksByTask,
        [created.id]: linkRowsToIds(created.id, links),
      },
    }));
  },

  branchTask: async (taskId: string) => {
    const source = get().tasks.find((task) => task.id === taskId);
    if (!source) return;
    const branchPrompt = `Branch from: ${source.prompt}`;
    await get().createTask(branchPrompt, { parentTaskId: taskId, referenceTaskIds: [taskId] });
  },

  deleteTask: async (taskId: string) => {
    await invoke("delete_task", { taskId });
    set((state) => {
      const nextTasks = state.tasks.filter((task) => task.id !== taskId);
      const nextArtifactsByTask = { ...state.artifactsByTask };
      const nextTaskLinksByTask = { ...state.taskLinksByTask };
      delete nextArtifactsByTask[taskId];
      delete nextTaskLinksByTask[taskId];
      for (const [id, linkedIds] of Object.entries(nextTaskLinksByTask)) {
        nextTaskLinksByTask[id] = linkedIds.filter((linkedId) => linkedId !== taskId);
      }
      runtimeEventBuffer.clearTask(taskId);
      return {
        tasks: nextTasks,
        selectedTaskId: state.selectedTaskId === taskId ? null : state.selectedTaskId,
        artifactsByTask: nextArtifactsByTask,
        taskLinksByTask: nextTaskLinksByTask,
      };
    });
  },

  linkTasks: async (taskId: string, relatedTaskId: string) => {
    if (taskId === relatedTaskId) return;
    await invoke("link_tasks", { taskId, relatedTaskId });
    const [linksA, linksB] = await Promise.all([
      invoke<TaskLinkRow[]>("list_task_links", { taskId }),
      invoke<TaskLinkRow[]>("list_task_links", { taskId: relatedTaskId }),
    ]);
    set((state) => ({
      taskLinksByTask: {
        ...state.taskLinksByTask,
        [taskId]: linkRowsToIds(taskId, linksA),
        [relatedTaskId]: linkRowsToIds(relatedTaskId, linksB),
      },
    }));
  },

  unlinkTasks: async (taskId: string, relatedTaskId: string) => {
    if (taskId === relatedTaskId) return;
    await invoke("unlink_tasks", { taskId, relatedTaskId });
    const [linksA, linksB] = await Promise.all([
      invoke<TaskLinkRow[]>("list_task_links", { taskId }),
      invoke<TaskLinkRow[]>("list_task_links", { taskId: relatedTaskId }),
    ]);
    set((state) => ({
      taskLinksByTask: {
        ...state.taskLinksByTask,
        [taskId]: linkRowsToIds(taskId, linksA),
        [relatedTaskId]: linkRowsToIds(relatedTaskId, linksB),
      },
    }));
  },

  startTask: async (taskId: string) => {
    const state = get();
    await invoke("run_plan_mode", {
      taskId,
      provider: state.selectedProvider,
      model: state.selectedModel,
    });
  },

  approvePlan: async (taskId: string) => {
    const state = get();
    await invoke("run_build_mode", {
      taskId,
      provider: state.selectedProvider,
      model: state.selectedModel,
    });
  },

  submitPlanFeedback: async (taskId: string, note: string) => {
    const state = get();
    await invoke("submit_plan_feedback", {
      taskId,
      note,
      provider: state.selectedProvider,
      model: state.selectedModel,
    });
  },

  cancelTask: async (taskId: string) => {
    await invoke("cancel_task", { taskId });
  },

  sendMessageToTask: async (taskId: string, message: string) => {
    const state = get();
    const task = state.tasks.find((t) => t.id === taskId);
    if (!task) throw new Error("Task not found");

    // Only allow messages to completed, failed, or cancelled tasks
    if (task.status !== "completed" && task.status !== "failed" && task.status !== "cancelled") {
      throw new Error(
        `Can only send follow-up messages to completed, failed, or cancelled tasks (current status: ${task.status})`
      );
    }

    await invoke("send_message_to_task", {
      taskId,
      message,
      provider: state.selectedProvider,
      model: state.selectedModel,
    });
  },

  selectTask: (taskId: string | null) => set({ selectedTaskId: taskId }),

  setProviderConfig: async (provider, apiKey, defaultModel, baseUrl) => {
    await invoke("set_provider_config", {
      provider,
      apiKey,
      defaultModel: defaultModel?.trim() ? defaultModel.trim() : null,
      baseUrl: baseUrl?.trim() ? baseUrl.trim() : null,
    });

    const providerConfigs = await invoke<ProviderConfigView[]>("get_provider_configs");
    const modelCatalog = get().modelCatalog;
    const selectedProvider = get().selectedProvider;
    const selectedModel =
      selectedProvider === provider
        ? modelForProvider(selectedProvider, providerConfigs, modelCatalog)
        : get().selectedModel;

    set({ providerConfigs, selectedModel });
  },

  selectProviderModel: (provider, model) => set({ selectedProvider: provider, selectedModel: model }),

  setWorkspaceRoot: async (workspaceRoot: string) => {
    await invoke("set_workspace_root", { workspaceRoot });
    set({ workspaceRoot });
    // Re-scan workspace skills for the new workspace
    await get().refreshWorkspaceSkills();
  },

  refreshWorkspaceRoot: async () => {
    const root = await invoke<WorkspaceRootView>("get_workspace_root");
    set({ workspaceRoot: root.workspace_root });
  },

  refreshTaskArtifacts: async (taskId: string) => {
    const latestRun = await invoke<RunRow | null>("get_latest_run", { taskId });
    if (!latestRun?.id) {
      set((state) => {
        const prev = state.artifactsByTask[taskId] ?? [];
        if (prev.length === 0) return state;
        return { artifactsByTask: { ...state.artifactsByTask, [taskId]: [] } };
      });
      return;
    }

    const artifacts = await invoke<ArtifactRow[]>("list_run_artifacts", { runId: latestRun.id });
    set((state) => {
      const prev = state.artifactsByTask[taskId] ?? [];
      const unchanged =
        prev.length === artifacts.length && prev.every((item, index) => item.id === artifacts[index]?.id);
      if (unchanged) return state;
      return { artifactsByTask: { ...state.artifactsByTask, [taskId]: artifacts } };
    });
  },

  refreshTaskLinks: async (taskId: string) => {
    const links = await invoke<TaskLinkRow[]>("list_task_links", { taskId });
    set((state) => ({
      taskLinksByTask: {
        ...state.taskLinksByTask,
        [taskId]: linkRowsToIds(taskId, links),
      },
    }));
  },

  refreshSkills: async () => {
    const skills = await invoke<SkillCatalogItem[]>("list_available_skills");
    set({ skills });
  },

  searchSkills: async (query: string, source?: string, limit?: number) => {
    return invoke<SkillCatalogItem[]>("search_skills", {
      query,
      source: source?.trim() ? source.trim() : null,
      limit: typeof limit === "number" ? limit : null,
    });
  },

  addCustomSkill: async (skill: NewCustomSkill) => {
    await invoke("add_custom_skill", { skill });
    await get().refreshSkills();
  },

  importContext7Skill: async (libraryId: string, title?: string) => {
    await invoke("import_context7_skill", {
      libraryId,
      title: title?.trim() ? title.trim() : null,
    });
    await get().refreshSkills();
  },

  importVercelSkill: async (skillName: string) => {
    await invoke("import_vercel_skill", { skillName });
    await get().refreshSkills();
  },

  removeSkill: async (skillId: string) => {
    await invoke("remove_custom_skill", { skillId });
    await get().refreshSkills();
  },

  refreshWorkspaceSkills: async () => {
    const workspaceSkills = await invoke<WorkspaceSkill[]>("list_workspace_skills");
    set({ workspaceSkills });
  },

  getWorkspaceSkillContent: async (skillId: string) => {
    return invoke<WorkspaceSkill>("get_workspace_skill_content", { skillId });
  },

  refreshMcpServers: async () => {
    const mcpServers = await invoke<McpServerConfig[]>("list_mcp_server_configs");
    set({ mcpServers });
  },

  upsertMcpServer: async (server: McpServerInput) => {
    await invoke("upsert_mcp_server_config", { input: server });
    await Promise.all([get().refreshMcpServers(), get().refreshMcpTools()]);
  },

  removeMcpServer: async (serverId: string) => {
    await invoke("remove_mcp_server_config", { serverId });
    await Promise.all([get().refreshMcpServers(), get().refreshMcpTools()]);
  },

  refreshMcpTools: async () => {
    try {
      const mcpTools = await invoke<McpToolEntry[]>("refresh_mcp_tools");
      set({ mcpTools });
    } catch {
      const mcpTools = await invoke<McpToolEntry[]>("list_cached_mcp_tools");
      set({ mcpTools });
    }
  },
}));
