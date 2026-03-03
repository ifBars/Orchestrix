import { useState } from "react";
import { SafeStreamdown } from "../messages/SafeStreamdown";
import { cn } from "@/lib/utils";
import { FileCode, ChevronDown, ChevronRight } from "lucide-react";
import type { SupportedLanguage } from "@/lib/codemirror/languages";

export interface ToolCodeContent {
  type: "file_read" | "file_write" | "patch_diff" | "code";
  language: SupportedLanguage;
  content: string;
  filename?: string;
  path?: string;
}

/**
 * Detects if a tool result contains code/file content that should be displayed
 * with syntax highlighting.
 */
export function detectCodeContent(
  toolName: string,
  toolArgs: Record<string, unknown> | undefined,
  toolResult: string | undefined
): ToolCodeContent | null {
  if (!toolResult) return null;

  // fs.read - returns file content
  if (toolName === "fs.read") {
    try {
      const result = JSON.parse(toolResult);
      if (result.content && typeof result.content === "string") {
        const path = result.path || toolArgs?.path || "";
        const filename = typeof path === "string" ? path.split(/[\\/]/).pop() : undefined;
        const language = detectLanguageFromFilename(filename || "");
        return {
          type: "file_read",
          language,
          content: result.content,
          filename,
          path: typeof path === "string" ? path : undefined,
        };
      }
    } catch {
      // Not JSON, treat as plain text
    }
  }

  // fs.write - the content is in the args, result is minimal
  if (toolName === "fs.write" && toolArgs) {
    const content = toolArgs.content;
    const path = toolArgs.path;
    if (typeof content === "string" && typeof path === "string") {
      const filename = path.split(/[\\/]/).pop();
      const language = detectLanguageFromFilename(filename || "");
      return {
        type: "file_write",
        language,
        content,
        filename,
        path,
      };
    }
  }

  // fs.patch - returns diffs
  if (toolName === "fs.patch") {
    try {
      const result = JSON.parse(toolResult);
      if (result.diffs && Array.isArray(result.diffs) && result.diffs.length > 0) {
        const diffs = result.diffs as string[];
        return {
          type: "patch_diff",
          language: "plain",
          content: diffs.join("\n\n"),
          filename: result.modified?.[0] || result.added?.[0],
        };
      }
    } catch {
      // Not JSON
    }
  }

  return null;
}

function detectLanguageFromFilename(filename: string): SupportedLanguage {
  const ext = filename.split(".").pop()?.toLowerCase() || "";
  const langMap: Record<string, SupportedLanguage> = {
    ts: "typescript",
    tsx: "tsx",
    js: "javascript",
    jsx: "jsx",
    rs: "rust",
    py: "python",
    go: "go",
    java: "java",
    kt: "java",
    cpp: "cpp",
    c: "cpp",
    h: "cpp",
    hpp: "cpp",
    cs: "csharp",
    rb: "plain",
    json: "json",
    yaml: "yaml",
    yml: "yaml",
    xml: "xml",
    html: "html",
    css: "css",
    scss: "css",
    sass: "css",
    less: "css",
    md: "markdown",
    sh: "shell",
    bash: "shell",
    zsh: "shell",
  };
  return langMap[ext] || "plain";
}

function getMarkdownLanguage(language: SupportedLanguage): string {
  const langMap: Record<SupportedLanguage, string> = {
    typescript: "typescript",
    tsx: "tsx",
    javascript: "javascript",
    jsx: "jsx",
    rust: "rust",
    python: "python",
    go: "go",
    java: "java",
    cpp: "cpp",
    csharp: "csharp",
    json: "json",
    yaml: "yaml",
    xml: "xml",
    html: "html",
    css: "css",
    markdown: "markdown",
    shell: "bash",
    plain: "",
  };
  return langMap[language] || "";
}

type ToolCodeDisplayProps = {
  codeContent: ToolCodeContent;
  className?: string;
  initiallyExpanded?: boolean;
};

export function ToolCodeDisplay({
  codeContent,
  className,
  initiallyExpanded = true,
}: ToolCodeDisplayProps) {
  const [expanded, setExpanded] = useState(initiallyExpanded);

  const typeLabel =
    codeContent.type === "file_read"
      ? "File Content"
      : codeContent.type === "file_write"
      ? "Written Content"
      : codeContent.type === "patch_diff"
      ? "Diff"
      : "Code";

  const markdownLanguage = getMarkdownLanguage(codeContent.language);
  const markdownContent = markdownLanguage
    ? `\`\`\`${markdownLanguage}\n${codeContent.content}\n\`\`\``
    : `\`\`\`\n${codeContent.content}\n\`\`\``;

  return (
    <div className={cn("rounded-lg border border-border/60 bg-card/30", className)}>
      <button
        type="button"
        aria-expanded={expanded}
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left transition-colors hover:bg-accent/30 rounded-t-lg"
      >
        <FileCode size={14} className="shrink-0 text-muted-foreground" />
        <span className="flex-1 text-xs font-medium text-foreground">
          {typeLabel}
          {codeContent.filename && (
            <span className="ml-2 text-muted-foreground">{codeContent.filename}</span>
          )}
        </span>
        {expanded ? (
          <ChevronDown size={14} className="shrink-0 text-muted-foreground" />
        ) : (
          <ChevronRight size={14} className="shrink-0 text-muted-foreground" />
        )}
      </button>

      {expanded && (
        <div className="border-t border-border/50 max-h-[400px] overflow-auto">
          <div className="prose prose-sm max-w-none p-3 text-foreground dark:prose-invert">
            <SafeStreamdown content={markdownContent} />
          </div>
        </div>
      )}
    </div>
  );
}