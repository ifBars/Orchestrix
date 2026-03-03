import { Component, useMemo, type ReactNode } from "react";
import { Streamdown, type BundledTheme } from "streamdown";
import { createCodePlugin } from "@streamdown/code";
import { createMermaidPlugin } from "@streamdown/mermaid";
import { useStreamTypewriter } from "@/hooks/useStreamTypewriter";
import { useTheme } from "@/contexts/ThemeContext";

// Singleton plugins — created once at module level for memoization stability (per streamdown docs)
const lightCodePlugin = createCodePlugin({
  themes: ["github-light", "github-dark"],
});

const darkCodePlugin = createCodePlugin({
  themes: ["github-dark", "github-light"],
});

// Mermaid plugins keyed by theme — also singletons
const lightMermaidPlugin = createMermaidPlugin({
  config: { theme: "default" },
});

const darkMermaidPlugin = createMermaidPlugin({
  config: { theme: "dark" },
});

// Frozen plugin sets — stable object references prevent Streamdown re-initialization
const PLUGINS_LIGHT = Object.freeze({ code: lightCodePlugin });
const PLUGINS_DARK = Object.freeze({ code: darkCodePlugin });
const PLUGINS_LIGHT_MERMAID = Object.freeze({ code: lightCodePlugin, mermaid: lightMermaidPlugin });
const PLUGINS_DARK_MERMAID = Object.freeze({ code: darkCodePlugin, mermaid: darkMermaidPlugin });

type SafeStreamdownProps = {
  content: string;
  isStreaming?: boolean;
  /** When true, renders mermaid diagrams in addition to code blocks */
  mermaid?: boolean;
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

export function SafeStreamdown({
  content,
  isStreaming = false,
  mermaid = false,
}: SafeStreamdownProps) {
  const { darkMode } = useTheme();
  const displayedContent = useStreamTypewriter(content, isStreaming);

  // Per memoization docs: plugins object must be stable — use pre-frozen singletons.
  // useMemo ensures the correct frozen set is selected without creating new objects.
  const plugins = useMemo(() => {
    if (mermaid) {
      return darkMode ? PLUGINS_DARK_MERMAID : PLUGINS_LIGHT_MERMAID;
    }
    return darkMode ? PLUGINS_DARK : PLUGINS_LIGHT;
  }, [darkMode, mermaid]);

  const shikiTheme: [BundledTheme, BundledTheme] = darkMode
    ? ["github-dark", "github-light"]
    : ["github-light", "github-dark"];

  return (
    <StreamdownBoundary content={displayedContent}>
      <Streamdown
        plugins={plugins}
        shikiTheme={shikiTheme}
        isAnimating={isStreaming}
      >
        {displayedContent}
      </Streamdown>
    </StreamdownBoundary>
  );
}
