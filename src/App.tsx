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
import { EmptyState } from "@/components/EmptyState";

const SETTINGS_SECTION_KEY = "orchestrix:last-settings-section";

const EMPTY_ARTIFACTS: readonly never[] = [];

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
  const [tasks, selectedTaskId, bootstrap, shutdown] = useAppStore(
    useShallow((state) => [state.tasks, state.selectedTaskId, state.bootstrap, state.shutdown])
  );

  const [activeView, setActiveView] = useState<"chat" | "settings">("chat");
  const [settingsSection, setSettingsSectionState] = useState<SettingsSectionId>(() => {
    const normalized = normalizeSettingsSection(localStorage.getItem(SETTINGS_SECTION_KEY));
    if (normalized) return normalized;
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

export default App;
