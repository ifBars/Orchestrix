/**
 * Live agent message stream state per task.
 */

import type { AgentMessageStream } from "./types";

export class AgentStreamState {
  private streamByTask = new Map<string, AgentMessageStream>();

  getStream(taskId: string): AgentMessageStream | null {
    return this.streamByTask.get(taskId) ?? null;
  }

  startStream(
    taskId: string,
    params: {
      streamId: string;
      createdAt: string;
      seq: number;
      subAgentId?: string;
      stepIdx?: number;
      turn?: number;
    }
  ): void {
    this.streamByTask.set(taskId, {
      streamId: params.streamId,
      content: "",
      startedAt: params.createdAt,
      updatedAt: params.createdAt,
      seq: params.seq,
      isStreaming: true,
      subAgentId: params.subAgentId,
      stepIdx: params.stepIdx,
      turn: params.turn,
    });
  }

  appendDelta(
    taskId: string,
    params: {
      streamId?: string;
      delta: string;
      createdAt: string;
      seq: number;
      subAgentId?: string;
      stepIdx?: number;
      turn?: number;
    }
  ): void {
    if (!params.delta) return;

    const existing = this.streamByTask.get(taskId);
    const shouldStartNew =
      !existing || (params.streamId != null && params.streamId.length > 0 && existing.streamId !== params.streamId);

    if (shouldStartNew) {
      const streamId =
        params.streamId && params.streamId.length > 0 ? params.streamId : `stream-${taskId}-${params.seq}`;
      this.startStream(taskId, {
        streamId,
        createdAt: params.createdAt,
        seq: params.seq,
        subAgentId: params.subAgentId,
        stepIdx: params.stepIdx,
        turn: params.turn,
      });
    }

    const current = this.streamByTask.get(taskId);
    if (!current) return;

    this.streamByTask.set(taskId, {
      ...current,
      content: `${current.content}${params.delta}`,
      updatedAt: params.createdAt,
      seq: params.seq,
      isStreaming: true,
      subAgentId: params.subAgentId ?? current.subAgentId,
      stepIdx: typeof params.stepIdx === "number" ? params.stepIdx : current.stepIdx,
      turn: typeof params.turn === "number" ? params.turn : current.turn,
    });
  }

  completeStream(taskId: string, streamId?: string, completedAt?: string, seq?: number): void {
    const current = this.streamByTask.get(taskId);
    if (!current) return;
    if (streamId && streamId.length > 0 && current.streamId !== streamId) return;

    this.streamByTask.set(taskId, {
      ...current,
      isStreaming: false,
      updatedAt: completedAt ?? current.updatedAt,
      seq: typeof seq === "number" ? seq : current.seq,
    });
  }

  clearStream(taskId: string): void {
    this.streamByTask.delete(taskId);
  }

  clearTask(taskId: string): void {
    this.streamByTask.delete(taskId);
  }
}
