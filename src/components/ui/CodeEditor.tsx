import { useEffect, useRef, useState } from "react";
import { EditorView, keymap } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { searchKeymap } from "@codemirror/search";
import { loadLanguage, type SupportedLanguage } from "@/lib/codemirror/languages";
import { orchestrixTheme, orchestrixSyntaxHighlighting } from "@/lib/codemirror/theme";
import { cn } from "@/lib/utils";

type CodeEditorProps = {
  value: string;
  onChange?: (value: string) => void;
  language?: SupportedLanguage;
  filename?: string;
  readOnly?: boolean;
  className?: string;
  minHeight?: string;
  maxHeight?: string;
  autoFocus?: boolean;
};

export function CodeEditor({
  value,
  onChange,
  language = "plain",
  filename,
  readOnly = false,
  className,
  minHeight = "200px",
  maxHeight,
  autoFocus = false,
}: CodeEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [loadedLanguage, setLoadedLanguage] = useState<SupportedLanguage | null>(null);

  // Detect language from filename if provided
  const detectedLanguage = filename
    ? (() => {
        const { getLanguageFromExtension } = require("@/lib/codemirror/languages");
        return getLanguageFromExtension(filename);
      })()
    : language;

  // Initialize editor
  useEffect(() => {
    if (!containerRef.current) return;

    let isMounted = true;

    const initEditor = async () => {
      setIsLoading(true);
      
      // Load language support
      const langExtension = detectedLanguage !== "plain" 
        ? await loadLanguage(detectedLanguage)
        : null;

      if (!isMounted) return;

      const extensions = [
        orchestrixTheme,
        orchestrixSyntaxHighlighting,
        EditorView.lineWrapping,
        keymap.of(searchKeymap),
        EditorView.theme({
          "&": {
            minHeight,
            maxHeight: maxHeight || "none",
          },
        }),
      ];

      if (langExtension) {
        extensions.push(langExtension);
      }

      if (readOnly) {
        extensions.push(EditorState.readOnly.of(true));
      }

      if (onChange) {
        extensions.push(
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              onChange(update.state.doc.toString());
            }
          })
        );
      }

      const state = EditorState.create({
        doc: value,
        extensions,
      });

      const view = new EditorView({
        state,
        parent: containerRef.current!,
      });

      viewRef.current = view;
      setLoadedLanguage(detectedLanguage);
      setIsLoading(false);

      if (autoFocus) {
        view.focus();
      }
    };

    initEditor();

    return () => {
      isMounted = false;
      if (viewRef.current) {
        viewRef.current.destroy();
        viewRef.current = null;
      }
    };
  }, []);

  // Update value when prop changes (but only if editor exists and content differs)
  useEffect(() => {
    const view = viewRef.current;
    if (!view || view.state.doc.toString() === value) return;
    
    view.dispatch({
      changes: {
        from: 0,
        to: view.state.doc.length,
        insert: value,
      },
    });
  }, [value]);



  return (
    <div className={cn("relative", className)}>
      {isLoading && (
        <div className="absolute inset-0 flex items-center justify-center bg-card/50 backdrop-blur-sm z-10">
          <div className="text-sm text-muted-foreground">Loading editor...</div>
        </div>
      )}
      <div 
        ref={containerRef}
        className={cn(
          "overflow-hidden rounded-md",
          readOnly && "cursor-default"
        )}
      />
      {loadedLanguage && loadedLanguage !== "plain" && (
        <div className="absolute top-2 right-2 px-2 py-1 text-xs text-muted-foreground bg-muted rounded">
          {loadedLanguage}
        </div>
      )}
    </div>
  );
}
