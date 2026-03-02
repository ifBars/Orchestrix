import type { LanguageSupport, StreamLanguage } from "@codemirror/language";

export type LanguageResult = LanguageSupport | ReturnType<typeof StreamLanguage.define> | null;

export type SupportedLanguage =
  | "javascript"
  | "typescript"
  | "jsx"
  | "tsx"
  | "rust"
  | "python"
  | "json"
  | "markdown"
  | "css"
  | "csharp"
  | "cpp"
  | "java"
  | "go"
  | "shell"
  | "yaml"
  | "xml"
  | "html"
  | "plain";

const extensionToLanguage: Record<string, SupportedLanguage> = {
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  ts: "typescript",
  jsx: "jsx",
  tsx: "tsx",
  rs: "rust",
  py: "python",
  json: "json",
  md: "markdown",
  mdx: "markdown",
  css: "css",
  scss: "css",
  sass: "css",
  less: "css",
  cs: "csharp",
  cpp: "cpp",
  cxx: "cpp",
  cc: "cpp",
  h: "cpp",
  hpp: "cpp",
  java: "java",
  go: "go",
  sh: "shell",
  bash: "shell",
  zsh: "shell",
  fish: "shell",
  ps1: "shell",
  yaml: "yaml",
  yml: "yaml",
  xml: "xml",
  html: "html",
  htm: "html",
  txt: "plain",
  log: "plain",
};

export function getLanguageFromExtension(filename: string): SupportedLanguage {
  const ext = filename.split(".").pop()?.toLowerCase() || "";
  return extensionToLanguage[ext] || "plain";
}

export function getLanguageFromMimeType(mimeType: string): SupportedLanguage {
  const typeMap: Record<string, SupportedLanguage> = {
    "application/javascript": "javascript",
    "application/typescript": "typescript",
    "text/javascript": "javascript",
    "text/typescript": "typescript",
    "text/x-rustsrc": "rust",
    "text/x-python": "python",
    "application/json": "json",
    "text/markdown": "markdown",
    "text/css": "css",
    "text/x-csharp": "csharp",
    "text/x-java-source": "java",
    "text/x-go": "go",
    "application/x-sh": "shell",
    "text/yaml": "yaml",
    "application/xml": "xml",
    "text/html": "html",
    "text/plain": "plain",
  };
  return typeMap[mimeType] || "plain";
}

export async function loadLanguage(
  lang: SupportedLanguage
): Promise<LanguageResult> {
  switch (lang) {
    case "javascript":
    case "jsx": {
      const { javascript } = await import("@codemirror/lang-javascript");
      return javascript({ jsx: true });
    }
    case "typescript":
    case "tsx": {
      const { javascript } = await import("@codemirror/lang-javascript");
      return javascript({ typescript: true, jsx: lang === "tsx" });
    }
    case "rust": {
      const { rust } = await import("@codemirror/lang-rust");
      return rust();
    }
    case "python": {
      const { python } = await import("@codemirror/lang-python");
      return python();
    }
    case "json": {
      const { json } = await import("@codemirror/lang-json");
      return json();
    }
    case "markdown": {
      const { markdown } = await import("@codemirror/lang-markdown");
      return markdown();
    }
    case "css": {
      const { css } = await import("@codemirror/lang-css");
      return css();
    }
    case "csharp": {
      const { csharp } = await import("@replit/codemirror-lang-csharp");
      return csharp();
    }
    case "cpp": {
      const { cpp } = await import("@codemirror/lang-cpp");
      return cpp();
    }
    case "java": {
      const { java } = await import("@codemirror/lang-java");
      return java();
    }
    case "go": {
      const { go } = await import("@codemirror/lang-go");
      return go();
    }
    case "shell": {
      const { StreamLanguage } = await import("@codemirror/language");
      const { shell } = await import("@codemirror/legacy-modes/mode/shell");
      return StreamLanguage.define(shell);
    }
    case "yaml": {
      const { StreamLanguage } = await import("@codemirror/language");
      const { yaml } = await import("@codemirror/legacy-modes/mode/yaml");
      return StreamLanguage.define(yaml);
    }
    case "xml": {
      const { xml } = await import("@codemirror/lang-xml");
      return xml();
    }
    case "html": {
      const { html } = await import("@codemirror/lang-html");
      return html();
    }
    case "plain":
    default:
      return null;
  }
}
