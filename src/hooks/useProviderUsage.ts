import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ProviderUsageSnapshotView } from "@/types";

const REFRESH_INTERVAL_MS = 120_000; // 2 minutes

interface ProviderUsageState {
  data: ProviderUsageSnapshotView[] | null;
  isLoading: boolean;
  error: string | null;
  lastUpdatedAt: Date | null;
}

export function useProviderUsage(provider?: string) {
  const [state, setState] = useState<ProviderUsageState>({
    data: null,
    isLoading: false,
    error: null,
    lastUpdatedAt: null,
  });

  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const isFetchingRef = useRef(false);

  const fetchUsage = useCallback(async () => {
    if (isFetchingRef.current) return;
    isFetchingRef.current = true;

    setState((prev) => ({ ...prev, isLoading: true, error: null }));

    try {
      const result = await invoke<ProviderUsageSnapshotView[]>(
        "get_provider_usage_snapshot",
        {
          provider: provider || null,
        }
      );

      setState({
        data: result,
        isLoading: false,
        error: null,
        lastUpdatedAt: new Date(),
      });
    } catch (error) {
      setState((prev) => ({
        ...prev,
        isLoading: false,
        error:
          error instanceof Error
            ? error.message
            : "Failed to fetch provider usage",
      }));
    } finally {
      isFetchingRef.current = false;
    }
  }, [provider]);

  const refresh = useCallback(async () => {
    await fetchUsage();
  }, [fetchUsage]);

  useEffect(() => {
    // Initial fetch
    fetchUsage();

    // Set up interval for auto-refresh
    intervalRef.current = setInterval(() => {
      fetchUsage();
    }, REFRESH_INTERVAL_MS);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [fetchUsage]);

  return {
    usage: state.data,
    isLoading: state.isLoading,
    error: state.error,
    lastUpdatedAt: state.lastUpdatedAt,
    refresh,
  };
}
