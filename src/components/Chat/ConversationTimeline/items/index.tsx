import { memo } from "react";
import { ErrorItem } from "./ErrorItem";
import { FileChangeItem } from "./FileChangeItem";
import { StatusChangeItem } from "./StatusChangeItem";
import { ThinkingItem } from "./ThinkingItem";
import { ToolCallItem } from "./ToolCallItem";
import { ToolCallBatchItem } from "./ToolCallBatchItem";
import { AgentMessageItem } from "../messages/AgentMessageItem";
import { UserMessageItem } from "../messages/UserMessageItem";
import type { ConversationItem } from "@/runtime/eventBuffer";

type ConversationItemViewProps = {
  item: ConversationItem;
};

export const ConversationItemView = memo(function ConversationItemView({ item }: ConversationItemViewProps) {
  switch (item.type) {
    case "userMessage":
      return <UserMessageItem content={item.content ?? ""} />;
    case "agentMessage":
      return <AgentMessageItem item={item} />;
    case "toolCall":
      return <ToolCallItem item={item} />;
    case "fileChange":
      return <FileChangeItem item={item} />;
    case "statusChange":
      return <StatusChangeItem item={item} />;
    case "error":
      return <ErrorItem item={item} />;
    case "thinking":
      return <ThinkingItem item={item} />;
    default:
      return null;
  }
});

export { ErrorItem, FileChangeItem, StatusChangeItem, ThinkingItem, ToolCallItem, ToolCallBatchItem };
