import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/shallow";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "@/stores/appStore";
import { ThemeContext } from "@/contexts/ThemeContext";
import { IdeShell } from "@/layouts/IdeShell";
import { Header } from "@/components/Header";
import { Sidebar } from "@/components/Sidebar";
import { ChatInterface } from "@/components/Chat/ChatInterface";
import { Composer } from "@/components/Composer";
import { ArtifactPanel } from "@/components/Artifacts/ArtifactPanel";
import { SettingsPage } from "@/components/Settings/SettingsPage";
import { BenchmarkPage } from "@/components/ModelBenchmarksPage";
import { BenchmarkWindowHeader } from "@/components/BenchmarkWindowHeader";
import { SETTINGS_SECTIONS, type SettingsSectionId } from "@/components/Settings/types";
import { EmptyState } from "@/components/EmptyState";
import { ArchitectureCanvas } from "@/components/Canvas";
import { CommandPalette } from "@/components/CommandPalette";
import { useUpdaterStore } from "@/stores/updaterStore";

const SETTINGS_SECTION_KEY = "orchestrix:last-settings-section";

const EMPTY_ARTIFACTS: readonly never[] = [];

type AppMode = "default" | "benchmark";

function resolveAppMode(): AppMode {
  const params = new URLSearchParams(window.location.search);
  const mode = params.get("mode")?.trim().toLowerCase();
  if (mode === "benchmark") {
    return "benchmark";
  }
  return "default";
}

function resolveInitialView(mode: AppMode): "chat" | "settings" | "benchmarks" {
  if (mode === "benchmark") {
    return "benchmarks";
  }

  const params = new URLSearchParams(window.location.search);
  const view = params.get("view")?.trim().toLowerCase();
  if (view === "settings") return "settings";

  const hash = window.location.hash.trim().toLowerCase();
  if (hash === "#settings") return "settings";
  return "chat";
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return target.isContentEditable || tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}

function normalizeSettingsSection(value: string | null): SettingsSectionId | null {
  if (!value) return null;
  if (value === "compaction") return "context";
  if (SETTINGS_SECTIONS.some((s) => s.id === value)) {
    return value as SettingsSectionId;
  }
  return null;
}

function App() {
  const appMode = useMemo(() => resolveAppMode(), []);
  const benchmarkOnlyMode = appMode === "benchmark";

  const [tasks, selectedTaskId, bootstrap, shutdown] = useAppStore(
    useShallow((state) => [state.tasks, state.selectedTaskId, state.bootstrap, state.shutdown])
  );
  const updaterBootstrap = useUpdaterStore((state) => state.bootstrap);

  const [activeView, setActiveView] = useState<"chat" | "settings" | "benchmarks">(
    () => resolveInitialView(appMode)
  );
  const [settingsSection, setSettingsSectionState] = useState<SettingsSectionId>(() => {
    const normalized = normalizeSettingsSection(localStorage.getItem(SETTINGS_SECTION_KEY));
    if (normalized) return normalized;
    return "general";
  });
  const [artifactsOpen, setArtifactsOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [darkMode, setDarkMode] = useState(true);
  const [chatActiveTab, setChatActiveTab] = useState<"chat" | "review" | "canvas">("chat");
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [canvasFocusNodeId, setCanvasFocusNodeId] = useState<string | null>(null);

  // Derive a view-safe tab for ChatInterface (it only understands "chat" | "review")
  const chatInterfaceTab: "chat" | "review" =
    chatActiveTab === "canvas" ? "chat" : chatActiveTab;
  const setChatInterfaceTab = (tab: "chat" | "review") => setChatActiveTab(tab);

  const setSettingsSection = useCallback((section: SettingsSectionId) => {
    setSettingsSectionState(section);
    localStorage.setItem(SETTINGS_SECTION_KEY, section);
  }, []);

  useEffect(() => {
    bootstrap().catch(console.error);
    return () => shutdown();
  }, [bootstrap, shutdown]);

  useEffect(() => {
    if (benchmarkOnlyMode) return;
    updaterBootstrap().catch(console.error);
  }, [benchmarkOnlyMode, updaterBootstrap]);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", darkMode);
  }, [darkMode]);

  const selectedTask = useMemo(
    () => tasks.find((task) => task.id === selectedTaskId) ?? null,
    [tasks, selectedTaskId]
  );

  // Auto-show artifacts only when a new task is first selected (not on every artifact update)
  const taskArtifacts = useAppStore(
    (state) => (selectedTaskId ? state.artifactsByTask[selectedTaskId] ?? EMPTY_ARTIFACTS : EMPTY_ARTIFACTS)
  );
  const previousTaskIdRef = useRef<string | null>(null);

  useEffect(() => {
    // Only auto-show when switching to a different task with artifacts
    if (selectedTaskId && selectedTaskId !== previousTaskIdRef.current && taskArtifacts.length > 0) {
      setArtifactsOpen(true);
    }
    previousTaskIdRef.current = selectedTaskId;
  }, [selectedTaskId, taskArtifacts.length]);

  // Keyboard shortcuts for navigation
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (isEditableTarget(e.target)) return;

      // Ctrl + 1 -> Chat
      if (e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey && e.code === "Digit1") {
        e.preventDefault();
        setActiveView("chat");
        return;
      }

      // Ctrl + 2 -> Settings
      if (e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey && e.code === "Digit2") {
        e.preventDefault();
        setActiveView("settings");
        return;
      }

      // Ctrl + 3 -> Benchmarks
      if (!benchmarkOnlyMode && e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey && e.code === "Digit3") {
        e.preventDefault();
        setActiveView("benchmarks");
        return;
      }

      // Ctrl + B -> Toggle sidebar
      if (e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey && e.code === "KeyB") {
        e.preventDefault();
        setSidebarOpen((prev) => !prev);
        return;
      }

      // Shift + [1..N] -> Jump to settings section
      if (!e.ctrlKey && !e.metaKey && !e.altKey && e.shiftKey && e.code.startsWith("Digit")) {
        const sectionIndex = Number(e.code.slice(5));
        if (!Number.isInteger(sectionIndex)) return;
        if (sectionIndex < 1 || sectionIndex > SETTINGS_SECTIONS.length) return;

        e.preventDefault();
        setActiveView("settings");
        setSettingsSection(SETTINGS_SECTIONS[sectionIndex - 1].id);
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [benchmarkOnlyMode, setSettingsSection]);

  // Listen for canvas navigation events from SafeStreamdown
  useEffect(() => {
    const handler = (e: CustomEvent<{ href: string; nodeId: string | null }>) => {
      // Switch to chat view and canvas tab
      setActiveView("chat");
      setChatActiveTab("canvas");
      // Store node ID for focusing in ArchitectureCanvas
      if (e.detail.nodeId) {
        setCanvasFocusNodeId(e.detail.nodeId);
      }
    };

    window.addEventListener("orchestrix:navigate-to-canvas", handler as EventListener);
    return () => window.removeEventListener("orchestrix:navigate-to-canvas", handler as EventListener);
  }, []);

  if (benchmarkOnlyMode) {
    return (
      <div className="flex h-screen w-screen flex-col overflow-hidden bg-background text-foreground">
        <header className="elevation-1 h-10 shrink-0 border-b border-border/80 bg-card/75 backdrop-blur-md">
          <BenchmarkWindowHeader />
        </header>
        <div className="min-h-0 flex-1">
          <BenchmarkPage />
        </div>
      </div>
    );
  }

  return (
    <ThemeContext.Provider value={{ darkMode }}>
      <IdeShell
        isArtifactsOpen={activeView === "chat" && artifactsOpen && chatActiveTab !== "canvas"}
        isSidebarOpen={sidebarOpen}
        onToggleSidebar={() => setSidebarOpen((prev) => !prev)}
        fillMain={activeView === "chat" && chatActiveTab === "canvas" && selectedTask != null}
        subheader={activeView === "chat" && selectedTask ? (
          <div className="flex items-center gap-0 border-b border-border/70 bg-card/60 px-4 backdrop-blur-md">
            {(["chat", "review", "canvas"] as const).map((tab) => (
              <button
                key={tab}
                type="button"
                onClick={() => setChatActiveTab(tab)}
                className={[
                  "relative px-3 py-2 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring",
                  chatActiveTab === tab
                    ? "text-foreground after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:bg-primary"
                    : "text-muted-foreground hover:text-foreground",
                ].join(" ")}
              >
                {tab.charAt(0).toUpperCase() + tab.slice(1)}
              </button>
            ))}
          </div>
        ) : null}
        header={
          <Header
            darkMode={darkMode}
            artifactsOpen={artifactsOpen}
            sidebarOpen={sidebarOpen}
            onToggleTheme={() => setDarkMode((prev) => !prev)}
            onToggleArtifacts={() => setArtifactsOpen((prev) => !prev)}
            onToggleSidebar={() => setSidebarOpen((prev) => !prev)}
            onOpenCommandPalette={() => setCommandPaletteOpen(true)}
          />
        }
        sidebar={
          <Sidebar
            activeView={activeView}
            activeSettingsSection={settingsSection}
            showBenchmarks={false}
            onOpenChat={() => setActiveView("chat")}
            onOpenSettings={(section) => {
              setActiveView("settings");
              if (section) {
                setSettingsSection(section);
              }
            }}
            onOpenBenchmarks={() => setActiveView("benchmarks")}
          />
        }
        main={activeView === "benchmarks" ? (
          <BenchmarkPage />
        ) : activeView === "settings" ? (
          <SettingsPage
            section={settingsSection}
            onBackToChat={() => setActiveView("chat")}
          />
        ) : selectedTask ? (
          chatActiveTab === "canvas" ? (
            <div className="h-full p-4">
              <ArchitectureCanvas 
                taskId={selectedTask.id} 
                focusNodeId={canvasFocusNodeId}
                onFocusComplete={() => setCanvasFocusNodeId(null)}
              />
            </div>
          ) : (
            <ChatInterface
              task={selectedTask}
              activeTab={chatInterfaceTab}
              onActiveTabChange={setChatInterfaceTab}
            />
          )
        ) : (
          <EmptyState />
        )}
        composer={activeView === "chat" && chatActiveTab !== "canvas" ? <Composer /> : null}
        artifacts={activeView === "chat" && selectedTask && chatActiveTab !== "canvas" ? (
          <ArtifactPanel
            taskId={selectedTask.id}
            onOpenReview={() => setChatActiveTab("review")}
          />
        ) : null}
      />

      <CommandPalette
        open={commandPaletteOpen}
        onOpenChange={setCommandPaletteOpen}
        onOpenChat={() => setActiveView("chat")}
        onOpenSettings={() => setActiveView("settings")}
        onOpenBenchmarks={() => setActiveView("benchmarks")}
        onNewConversation={() => {
          setActiveView("chat");
          const newButton = document.querySelector('[data-sidebar-action="new-conversation"]');
          if (newButton instanceof HTMLButtonElement) {
            newButton.click();
          }
        }}
        onSelectWorkspace={async () => {
          const selected = await openDialog({
            directory: true,
            title: "Select workspace folder",
          });
          if (typeof selected === "string" && selected.length > 0) {
            await useAppStore.getState().setWorkspaceRoot(selected);
          }
        }}
        darkMode={darkMode}
        onToggleTheme={() => setDarkMode((prev) => !prev)}
        artifactsOpen={artifactsOpen}
        onToggleArtifacts={() => setArtifactsOpen((prev) => !prev)}
      />
    </ThemeContext.Provider>
  );
}

export default App;
