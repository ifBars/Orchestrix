import { Component, useMemo, useCallback, type ReactNode } from "react";
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

// Canvas reference pattern: <@orchestrix_canvas> or <@orchestrix_canvas:nodeId>
const CANVAS_REF_PATTERN = /<@orchestrix_canvas(?::([^><]+))?>/g;

/**
 * Pre-process content to transform canvas references into markdown links.
 * <@orchestrix_canvas> -> [Architecture Canvas](orchestrix://canvas)
 * <@orchestrix_canvas:nodeId> -> [nodeId](orchestrix://canvas/nodeId)
 */
function preprocessCanvasRefs(content: string): string {
  return content.replace(CANVAS_REF_PATTERN, (_match, nodeId) => {
    if (nodeId) {
      const decodedNodeId = decodeURIComponent(nodeId.trim());
      return `[${decodedNodeId}](orchestrix://canvas/${encodeURIComponent(decodedNodeId)})`;
    }
    return `[Architecture Canvas](orchestrix://canvas)`;
  });
}

type SafeStreamdownProps = {
  content: string;
  isStreaming?: boolean;
  /** When true, renders mermaid diagrams in addition to code blocks */
  mermaid?: boolean;
  /** When true, enables canvas reference parsing (<@orchestrix_canvas>) */
  enableCanvasRefs?: boolean;
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
  enableCanvasRefs = true,
}: SafeStreamdownProps) {
  const { darkMode } = useTheme();
  const displayedContent = useStreamTypewriter(content, isStreaming);

  // Pre-process content to transform canvas references
  const processedContent = useMemo(() => {
    if (!enableCanvasRefs) return displayedContent;
    return preprocessCanvasRefs(displayedContent);
  }, [displayedContent, enableCanvasRefs]);

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

  // Handle clicks on canvas reference links
  const handleClick = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement;
    const anchor = target.closest('a[href^="orchestrix://canvas"]');
    if (anchor) {
      event.preventDefault();
      const href = anchor.getAttribute("href");
      if (href) {
        // Parse nodeId from href if present
        const url = new URL(href);
        const pathParts = url.pathname.split('/').filter(Boolean);
        const nodeId = pathParts.length > 0 ? decodeURIComponent(pathParts[0]) : null;
        
        // Dispatch custom event for canvas navigation
        const canvasEvent = new CustomEvent("orchestrix:navigate-to-canvas", {
          detail: { href, nodeId },
          bubbles: true,
        });
        window.dispatchEvent(canvasEvent);
      }
    }
  }, []);

  return (
    <StreamdownBoundary content={processedContent}>
      <div onClick={handleClick} className="contents">
        <Streamdown
          plugins={plugins}
          shikiTheme={shikiTheme}
          isAnimating={isStreaming}
        >
          {processedContent}
        </Streamdown>
      </div>
    </StreamdownBoundary>
  );
}
