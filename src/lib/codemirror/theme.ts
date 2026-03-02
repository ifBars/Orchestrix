import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";

export const orchestrixTheme = EditorView.theme(
  {
    "&": {
      backgroundColor: "var(--card)",
      color: "var(--foreground)",
      fontSize: "0.875rem",
      fontFamily: "var(--font-mono)",
      borderRadius: "var(--radius-md)",
      border: "1px solid var(--border)",
    },
    ".cm-content": {
      caretColor: "var(--primary)",
      padding: "12px 0",
    },
    
    ".cm-line": {
      padding: "0 12px",
      lineHeight: "1.5",
    },
    
    "&.cm-focused .cm-cursor": {
      borderLeftColor: "var(--primary)",
    },
    
    "&.cm-focused .cm-selectionBackground, ::selection": {
      backgroundColor: "var(--accent)",
      opacity: "0.4",
    },
    
    ".cm-gutters": {
      backgroundColor: "var(--muted)",
      color: "var(--muted-foreground)",
      border: "none",
      borderRight: "1px solid var(--border)",
      paddingRight: "8px",
      paddingLeft: "4px",
    },
    
    ".cm-activeLineGutter": {
      backgroundColor: "var(--accent)",
      color: "var(--foreground)",
    },
    
    ".cm-activeLine": {
      backgroundColor: "var(--accent)",
    },
    
    ".cm-tooltip": {
      backgroundColor: "var(--popover)",
      border: "1px solid var(--border)",
      borderRadius: "var(--radius-md)",
      boxShadow: "var(--shadow-2)",
    },
    
    ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": {
      backgroundColor: "var(--accent)",
      color: "var(--foreground)",
    },
    
    ".cm-searchMatch": {
      backgroundColor: "var(--warning)",
      opacity: "0.3",
    },
    
    ".cm-searchMatch.cm-searchMatch-selected": {
      backgroundColor: "var(--warning)",
      opacity: "0.6",
    },
    
    ".cm-selectionMatch": {
      backgroundColor: "var(--info)",
      opacity: "0.2",
    },
    
    ".cm-panels": {
      backgroundColor: "var(--card)",
      color: "var(--foreground)",
      borderTop: "1px solid var(--border)",
    },
    
    ".cm-button": {
      backgroundColor: "var(--secondary)",
      color: "var(--secondary-foreground)",
      border: "1px solid var(--border)",
      borderRadius: "var(--radius-sm)",
      padding: "4px 12px",
      fontSize: "0.75rem",
      cursor: "pointer",
    },
    
    ".cm-button:hover": {
      backgroundColor: "var(--accent)",
    },
    
    ".cm-textfield": {
      backgroundColor: "var(--input)",
      color: "var(--foreground)",
      border: "1px solid var(--border)",
      borderRadius: "var(--radius-sm)",
      padding: "4px 8px",
    },
    
    ".cm-foldPlaceholder": {
      backgroundColor: "var(--muted)",
      border: "1px solid var(--border)",
      color: "var(--muted-foreground)",
      borderRadius: "var(--radius-sm)",
    },
    
    ".cm-foldGutter span": {
      color: "var(--muted-foreground)",
      fontSize: "0.7rem",
    },
  },
  { dark: false }
);

export const orchestrixHighlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: "var(--primary)" },
  { tag: tags.operator, color: "var(--foreground)" },
  { tag: tags.className, color: "var(--info)" },
  { tag: tags.definition(tags.typeName), color: "var(--info)" },
  { tag: tags.typeName, color: "var(--info)" },
  { tag: tags.tagName, color: "var(--warning)" },
  { tag: tags.attributeName, color: "var(--success)" },
  { tag: tags.number, color: "var(--destructive)" },
  { tag: tags.string, color: "var(--success)" },
  { tag: tags.comment, color: "var(--muted-foreground)", fontStyle: "italic" },
  { tag: tags.variableName, color: "var(--foreground)" },
  { tag: tags.propertyName, color: "var(--foreground)" },
  { tag: tags.function(tags.variableName), color: "var(--accent)" },
]);

export const orchestrixSyntaxHighlighting = syntaxHighlighting(orchestrixHighlightStyle);
