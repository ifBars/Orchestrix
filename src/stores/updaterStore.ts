import { getVersion } from "@tauri-apps/api/app";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { create } from "zustand";

export type UpdaterStatus =
  | "idle"
  | "disabled"
  | "checking"
  | "available"
  | "up-to-date"
  | "downloading"
  | "installing"
  | "restarting"
  | "error";

export type PendingUpdateView = {
  version: string;
  publishedAt: string | null;
  notes: string | null;
};

type UpdateCheckOptions = {
  interactive?: boolean;
  promptOnAvailable?: boolean;
};

type UpdaterState = {
  bootstrapped: boolean;
  currentVersion: string | null;
  pendingUpdate: PendingUpdateView | null;
  status: UpdaterStatus;
  lastCheckedAt: string | null;
  downloadedBytes: number;
  contentLength: number | null;
  error: string | null;
  bootstrap: () => Promise<void>;
  checkForUpdates: (options?: UpdateCheckOptions) => Promise<PendingUpdateView | null>;
  installUpdate: () => Promise<boolean>;
  resetError: () => void;
};

let pendingUpdateHandle: Update | null = null;

function closePendingUpdateHandle() {
  if (!pendingUpdateHandle) return;
  void pendingUpdateHandle.close().catch(() => undefined);
  pendingUpdateHandle = null;
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function toPendingUpdate(update: Update): PendingUpdateView {
  return {
    version: update.version,
    publishedAt: update.date ?? null,
    notes: update.body ?? null,
  };
}

function buildUpdatePrompt(update: PendingUpdateView): string {
  const summary = update.notes
    ?.split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);

  if (summary) {
    return `Orchestrix v${update.version} is available.\n\n${summary}\n\nInstall now?`;
  }

  return `Orchestrix v${update.version} is available.\n\nInstall now?`;
}

export const useUpdaterStore = create<UpdaterState>((set, get) => ({
  bootstrapped: false,
  currentVersion: null,
  pendingUpdate: null,
  status: import.meta.env.DEV ? "disabled" : "idle",
  lastCheckedAt: null,
  downloadedBytes: 0,
  contentLength: null,
  error: null,

  bootstrap: async () => {
    if (get().bootstrapped) return;

    set({ bootstrapped: true, error: null });

    try {
      const currentVersion = await getVersion();
      set({
        currentVersion,
        status: import.meta.env.DEV ? "disabled" : "idle",
      });
    } catch (error) {
      set({
        status: "error",
        error: describeError(error),
      });
      return;
    }

    if (import.meta.env.DEV) {
      return;
    }

    await get().checkForUpdates({ interactive: false, promptOnAvailable: true });
  },

  checkForUpdates: async ({
    interactive = false,
    promptOnAvailable = interactive,
  }: UpdateCheckOptions = {}) => {
    if (import.meta.env.DEV) {
      set({
        status: "disabled",
        error: null,
      });

      if (interactive) {
        await message("Updater checks are disabled while Orchestrix is running in development mode.", {
          title: "Updater Disabled",
          kind: "info",
        });
      }

      return null;
    }

    closePendingUpdateHandle();
    set({
      status: "checking",
      error: null,
      downloadedBytes: 0,
      contentLength: null,
    });

    try {
      const update = await check();
      const lastCheckedAt = new Date().toISOString();

      if (!update) {
        set({
          pendingUpdate: null,
          status: "up-to-date",
          lastCheckedAt,
          error: null,
          downloadedBytes: 0,
          contentLength: null,
        });

        if (interactive) {
          await message("Orchestrix is already on the latest stable release.", {
            title: "No Update Available",
            kind: "info",
          });
        }

        return null;
      }

      pendingUpdateHandle = update;
      const pendingUpdate = toPendingUpdate(update);

      set({
        pendingUpdate,
        status: "available",
        lastCheckedAt,
        error: null,
        downloadedBytes: 0,
        contentLength: null,
      });

      if (promptOnAvailable) {
        const shouldInstall = await ask(buildUpdatePrompt(pendingUpdate), {
          title: "Update Available",
          kind: "info",
          okLabel: "Install",
          cancelLabel: "Later",
        });

        if (shouldInstall) {
          await get().installUpdate();
        }
      }

      return pendingUpdate;
    } catch (error) {
      closePendingUpdateHandle();

      const errorMessage = describeError(error);
      set({
        status: "error",
        error: errorMessage,
        lastCheckedAt: new Date().toISOString(),
      });

      if (interactive) {
        await message(`Failed to check GitHub Releases for an update.\n\n${errorMessage}`, {
          title: "Update Check Failed",
          kind: "error",
        });
      }

      return null;
    }
  },

  installUpdate: async () => {
    const cachedUpdate = pendingUpdateHandle;
    const pendingUpdate = get().pendingUpdate;

    if (!cachedUpdate || !pendingUpdate) {
      return false;
    }

    set({
      status: "downloading",
      error: null,
      downloadedBytes: 0,
      contentLength: null,
    });

    try {
      await cachedUpdate.downloadAndInstall((event) => {
        set((state) => {
          switch (event.event) {
            case "Started":
              return {
                status: "downloading" as const,
                downloadedBytes: 0,
                contentLength: event.data.contentLength ?? null,
              };
            case "Progress":
              return {
                status: "downloading" as const,
                downloadedBytes: state.downloadedBytes + event.data.chunkLength,
              };
            case "Finished":
              return {
                status: "installing" as const,
              };
            default:
              return state;
          }
        });
      });

      closePendingUpdateHandle();
      set({
        status: "restarting",
        pendingUpdate: null,
        error: null,
      });

      await message(
        `Orchestrix v${pendingUpdate.version} is installed. The app will restart to finish the update.`,
        {
          title: "Update Installed",
          kind: "info",
        },
      );

      await relaunch();
      return true;
    } catch (error) {
      const errorMessage = describeError(error);

      set({
        status: "error",
        error: errorMessage,
      });

      await message(`Failed to install the update.\n\n${errorMessage}`, {
        title: "Update Failed",
        kind: "error",
      });

      return false;
    }
  },

  resetError: () => set({ error: null }),
}));
