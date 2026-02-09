import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { IdeShell } from "@/layouts/IdeShell";
import { Header } from "@/components/Header";
import { Sidebar } from "@/components/Sidebar";
import { ChatInterface } from "@/components/Chat/ChatInterface";
import { Composer } from "@/components/Composer";
import { ArtifactPanel } from "@/components/Artifacts/ArtifactPanel";
import { SettingsSheet } from "@/components/Settings/SettingsSheet";
import { SkillsSheet } from "@/components/Settings/SkillsSheet";
import appIcon from "../src-tauri/icons/icon.png";

const EMPTY_ARTIFACTS: readonly never[] = [];

function App() {
  const [tasks, selectedTaskId, bootstrap, shutdown] = useAppStore(
    useShallow((state) => [state.tasks, state.selectedTaskId, state.bootstrap, state.shutdown])
  );

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [skillsOpen, setSkillsOpen] = useState(false);
  const [artifactsOpen, setArtifactsOpen] = useState(false);
  const [darkMode, setDarkMode] = useState(true);

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

  return (
    <>
      <IdeShell
        isArtifactsOpen={artifactsOpen}
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
            onOpenSettings={() => setSettingsOpen(true)}
            onOpenSkills={() => setSkillsOpen(true)}
          />
        }
        main={selectedTask ? <ChatInterface task={selectedTask} /> : <EmptyState />}
        composer={<Composer />}
        artifacts={selectedTask ? <ArtifactPanel taskId={selectedTask.id} /> : null}
      />

      <SettingsSheet open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <SkillsSheet open={skillsOpen} onClose={() => setSkillsOpen(false)} />
    </>
  );
}

function EmptyState() {
  return (
    <div className="flex h-full items-center justify-center">
      <div className="max-w-md text-center">
        <div className="mx-auto mb-5 h-12 w-12">
          <img src={appIcon} alt="Orchestrix" className="h-full w-full object-contain" />
        </div>
        <h1 className="text-2xl font-semibold tracking-tight">Orchestrix</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          Describe what you want to build. The agent will plan, execute tools, and produce artifacts.
        </p>
      </div>
    </div>
  );
}

export default App;
