import { create } from "zustand";

type TickMaps = Record<string, number>;

type StreamTickState = {
  planTickByTask: TickMaps;
  timelineTickByTask: TickMaps;
  bumpPlan: (taskId: string) => void;
  bumpTimeline: (taskId: string) => void;
  bumpBatch: (planTaskIds: Set<string>, timelineTaskIds: Set<string>) => void;
  resetTask: (taskId: string) => void;
};

export const useStreamTickStore = create<StreamTickState>((set) => ({
  planTickByTask: {},
  timelineTickByTask: {},
  bumpPlan: (taskId) =>
    set((state) => ({
      planTickByTask: {
        ...state.planTickByTask,
        [taskId]: (state.planTickByTask[taskId] ?? 0) + 1,
      },
    })),
  bumpTimeline: (taskId) =>
    set((state) => ({
      timelineTickByTask: {
        ...state.timelineTickByTask,
        [taskId]: (state.timelineTickByTask[taskId] ?? 0) + 1,
      },
    })),
  bumpBatch: (planTaskIds, timelineTaskIds) =>
    set((state) => {
      if (planTaskIds.size === 0 && timelineTaskIds.size === 0) {
        return state;
      }

      const nextPlan = { ...state.planTickByTask };
      for (const taskId of planTaskIds) {
        nextPlan[taskId] = (nextPlan[taskId] ?? 0) + 1;
      }

      const nextTimeline = { ...state.timelineTickByTask };
      for (const taskId of timelineTaskIds) {
        nextTimeline[taskId] = (nextTimeline[taskId] ?? 0) + 1;
      }

      return {
        planTickByTask: nextPlan,
        timelineTickByTask: nextTimeline,
      };
    }),
  resetTask: (taskId) =>
    set((state) => {
      const nextPlan = { ...state.planTickByTask };
      const nextTimeline = { ...state.timelineTickByTask };
      delete nextPlan[taskId];
      delete nextTimeline[taskId];
      return {
        planTickByTask: nextPlan,
        timelineTickByTask: nextTimeline,
      };
    }),
}));

export const useTaskPlanTick = (taskId: string | null) =>
  useStreamTickStore((state) => (taskId ? state.planTickByTask[taskId] ?? 0 : 0));

export const useTaskTimelineTick = (taskId: string | null) =>
  useStreamTickStore((state) => (taskId ? state.timelineTickByTask[taskId] ?? 0 : 0));
