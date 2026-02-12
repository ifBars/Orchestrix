import { useCallback, useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { IdeShell } from "@/layouts/IdeShell";
import { Header } from "@/components/Header";
import { Sidebar } from "@/components/Sidebar";
import { ChatInterface } from "@/components/Chat/ChatInterface";
import { Composer } from "@/components/Composer";
import { ArtifactPanel } from "@/components/Artifacts/ArtifactPanel";
import { SettingsPage } from "@/components/Settings/SettingsPage";
import { SETTINGS_SECTIONS, type SettingsSectionId } from "@/components/Settings/types";
import appIcon from "../src-tauri/icons/icon.png";

const SETTINGS_SECTION_KEY = "orchestrix:last-settings-section";

const EMPTY_ARTIFACTS: readonly never[] = [];

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return target.isContentEditable || tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}

function App() {
  const [tasks, selectedTaskId, bootstrap, shutdown] = useAppStore(
    useShallow((state) => [state.tasks, state.selectedTaskId, state.bootstrap, state.shutdown])
  );

  const [activeView, setActiveView] = useState<"chat" | "settings">("chat");
  const [settingsSection, setSettingsSectionState] = useState<SettingsSectionId>(() => {
    const saved = localStorage.getItem(SETTINGS_SECTION_KEY);
    if (saved && SETTINGS_SECTIONS.some((s) => s.id === saved)) {
      return saved as SettingsSectionId;
    }
    return "general";
  });
  const [artifactsOpen, setArtifactsOpen] = useState(false);
  const [darkMode, setDarkMode] = useState(true);
  const [chatActiveTab, setChatActiveTab] = useState<"chat" | "review">("chat");

  const setSettingsSection = useCallback((section: SettingsSectionId) => {
    setSettingsSectionState(section);
    localStorage.setItem(SETTINGS_SECTION_KEY, section);
  }, []);

  useEffect(() => {
    bootstrap().catch(console.error);
    return () => shutdown();
  }, [bootstrap, shutdown]);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", darkMode);
  }, [darkMode]);

  const selectedTask = useMemo(
    () => tasks.find((task) => task.id === selectedTaskId) ?? null,
    [tasks, selectedTaskId]
  );

  // Auto-show artifacts when a task is selected and has artifacts
  const taskArtifacts = useAppStore(
    (state) => (selectedTaskId ? state.artifactsByTask[selectedTaskId] ?? EMPTY_ARTIFACTS : EMPTY_ARTIFACTS)
  );

  useEffect(() => {
    if (taskArtifacts.length > 0 && !artifactsOpen) {
      setArtifactsOpen(true);
    }
  }, [artifactsOpen, taskArtifacts.length]);

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
  }, [setSettingsSection]);

  return (
    <>
      <IdeShell
        isArtifactsOpen={activeView === "chat" && artifactsOpen}
        header={
          <Header
            darkMode={darkMode}
            artifactsOpen={artifactsOpen}
            onToggleTheme={() => setDarkMode((prev) => !prev)}
            onToggleArtifacts={() => setArtifactsOpen((prev) => !prev)}
          />
        }
        sidebar={
          <Sidebar
            activeView={activeView}
            activeSettingsSection={settingsSection}
            onOpenChat={() => setActiveView("chat")}
            onOpenSettings={(section) => {
              setActiveView("settings");
              if (section) {
                setSettingsSection(section);
              }
            }}
          />
        }
        main={activeView === "settings" ? (
          <SettingsPage
            section={settingsSection}
            onBackToChat={() => setActiveView("chat")}
          />
        ) : selectedTask ? (
          <ChatInterface
            task={selectedTask}
            activeTab={chatActiveTab}
            onActiveTabChange={setChatActiveTab}
          />
        ) : (
          <EmptyState />
        )}
        composer={activeView === "chat" ? <Composer /> : null}
        artifacts={activeView === "chat" && selectedTask ? (
          <ArtifactPanel 
            taskId={selectedTask.id} 
            onOpenReview={() => setChatActiveTab("review")}
          />
        ) : null}
      />
    </>
  );
}

function EmptyState() {
  return (
    <div className="flex h-full items-center justify-center">
      <div className="max-w-xl rounded-2xl border border-border/70 bg-card/70 p-8 text-center elevation-2 backdrop-blur-sm">
        <div className="mx-auto mb-4 h-12 w-12 rounded-xl border border-border/70 bg-background/80 p-2">
          <img src={appIcon} alt="Orchestrix" className="h-full w-full object-contain" />
        </div>
        <p className="text-[11px] font-semibold uppercase tracking-widest text-muted-foreground/70">Ready</p>
        <h1 className="mt-2 text-2xl font-semibold tracking-tight">Start an orchestrated run</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          Describe your goal in one message. Orchestrix plans first, executes with full tool visibility, and keeps review in the loop.
        </p>
        <div className="mt-4 flex flex-wrap items-center justify-center gap-2 text-[11px] text-muted-foreground">
          <span className="rounded-full border border-border/70 bg-background/70 px-2.5 py-1">Plan + Build workflow</span>
          <span className="rounded-full border border-border/70 bg-background/70 px-2.5 py-1">Condensed timeline</span>
          <span className="rounded-full border border-border/70 bg-background/70 px-2.5 py-1">Artifact review</span>
        </div>
      </div>
    </div>
  );
}

export default App;
