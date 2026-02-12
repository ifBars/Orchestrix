import { Component, type ReactNode } from "react";
import { Streamdown } from "streamdown";
import { code } from "@streamdown/code";
import { useStreamTypewriter } from "@/hooks/useStreamTypewriter";

const STREAMDOWN_PLUGINS = Object.freeze({ code });

type SafeStreamdownProps = {
  content: string;
  isStreaming?: boolean;
};

type StreamdownBoundaryProps = {
  content: string;
  children: ReactNode;
};

type StreamdownBoundaryState = {
  failed: boolean;
};

class StreamdownBoundary extends Component<StreamdownBoundaryProps, StreamdownBoundaryState> {
  state: StreamdownBoundaryState = { failed: false };

  static getDerivedStateFromError(): StreamdownBoundaryState {
    return { failed: true };
  }

  componentDidCatch(error: Error) {
    console.error("Streamdown render error", error);
  }

  render() {
    if (this.state.failed) {
      return (
        <pre className="whitespace-pre-wrap break-words font-mono text-xs text-muted-foreground">
          {this.props.content}
        </pre>
      );
    }

    return this.props.children;
  }
}

export function SafeStreamdown({ content, isStreaming = false }: SafeStreamdownProps) {
  const displayedContent = useStreamTypewriter(content, isStreaming);

  return (
    <StreamdownBoundary content={displayedContent}>
      <Streamdown plugins={STREAMDOWN_PLUGINS}>{displayedContent}</Streamdown>
    </StreamdownBoundary>
  );
}
