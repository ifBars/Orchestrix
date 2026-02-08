import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ArtifactContentView, ArtifactRow } from "@/types";

export type ReviewComment = {
  id: string;
  line: number;
  text: string;
};

export function useArtifactReview(taskId: string, taskStatus: string, artifactsByTask: Record<string, ArtifactRow[]>) {
  const [selectedArtifactPath, setSelectedArtifactPath] = useState<string | null>(null);
  const [artifactPreview, setArtifactPreview] = useState<ArtifactContentView | null>(null);
  const [commentsByArtifact, setCommentsByArtifact] = useState<Record<string, ReviewComment[]>>({});
  const [draftLine, setDraftLine] = useState<number | null>(null);
  const [draftText, setDraftText] = useState("");
  const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
  const [generalReviewText, setGeneralReviewText] = useState("");
  const [showGeneralReviewInput, setShowGeneralReviewInput] = useState(false);
  const [draftAnchorTop, setDraftAnchorTop] = useState(120);

  const draftTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const reviewViewportRef = useRef<HTMLDivElement | null>(null);
  const lineButtonRefs = useRef<Record<number, HTMLButtonElement | null>>({});

  const markdownArtifacts = useMemo(() => {
    const all = artifactsByTask[taskId] ?? [];
    return all.filter((artifact) => {
      const path = artifact.uri_or_content.toLowerCase();
      return path.endsWith(".md") || path.endsWith(".markdown") || path.endsWith(".mdx");
    });
  }, [artifactsByTask, taskId]);

  useEffect(() => {
    if (taskStatus !== "awaiting_review" && taskStatus !== "planning") {
      setShowGeneralReviewInput(false);
    }
  }, [taskStatus]);

  useEffect(() => {
    if (markdownArtifacts.length === 0) {
      setSelectedArtifactPath(null);
      setArtifactPreview(null);
      return;
    }

    const current = selectedArtifactPath;
    const stillExists = current && markdownArtifacts.some((a) => a.uri_or_content === current);
    if (!stillExists) {
      setSelectedArtifactPath(markdownArtifacts[0].uri_or_content);
    }
  }, [markdownArtifacts, selectedArtifactPath]);

  useEffect(() => {
    if (!selectedArtifactPath) {
      setArtifactPreview(null);
      return;
    }

    invoke<ArtifactContentView>("read_artifact_content", { path: selectedArtifactPath })
      .then(setArtifactPreview)
      .catch(() =>
        setArtifactPreview({
          path: selectedArtifactPath,
          content: "Failed to load artifact",
          is_markdown: false,
        })
      );
  }, [selectedArtifactPath]);

  const previewText = artifactPreview?.content ?? "";
  const previewLines = previewText.split("\n");
  const activeComments = selectedArtifactPath ? commentsByArtifact[selectedArtifactPath] ?? [] : [];

  const openCommentEditor = (line: number) => {
    const existing = activeComments.find((comment) => comment.line === line);
    setDraftLine(line);
    setEditingCommentId(existing?.id ?? null);
    setDraftText(existing?.text ?? "");

    const lineButton = lineButtonRefs.current[line];
    const viewport = reviewViewportRef.current;
    if (lineButton && viewport) {
      const top = lineButton.offsetTop - viewport.scrollTop;
      setDraftAnchorTop(Math.max(84, top));
    }
  };

  useEffect(() => {
    if (draftLine == null) return;
    draftTextareaRef.current?.focus();
    draftTextareaRef.current?.setSelectionRange(
      draftTextareaRef.current.value.length,
      draftTextareaRef.current.value.length
    );
  }, [draftLine]);

  const saveComment = () => {
    if (!selectedArtifactPath || draftLine == null || !draftText.trim()) return;

    setCommentsByArtifact((prev) => {
      const current = prev[selectedArtifactPath] ?? [];
      const next = [...current];
      const idx = next.findIndex((comment) => comment.id === editingCommentId);
      if (idx >= 0) {
        next[idx] = { ...next[idx], line: draftLine, text: draftText.trim() };
      } else {
        next.push({
          id: crypto.randomUUID(),
          line: draftLine,
          text: draftText.trim(),
        });
      }
      next.sort((a, b) => a.line - b.line);
      return { ...prev, [selectedArtifactPath]: next };
    });

    setDraftLine(null);
    setDraftText("");
    setEditingCommentId(null);
  };

  const deleteComment = (commentId: string) => {
    if (!selectedArtifactPath) return;
    setCommentsByArtifact((prev) => {
      const current = prev[selectedArtifactPath] ?? [];
      return {
        ...prev,
        [selectedArtifactPath]: current.filter((comment) => comment.id !== commentId),
      };
    });
  };

  const startEditingComment = (commentId: string) => {
    const comment = activeComments.find((entry) => entry.id === commentId);
    if (!comment) return;
    setDraftLine(comment.line);
    setDraftText(comment.text);
    setEditingCommentId(comment.id);
  };

  const cancelDraft = () => {
    setDraftLine(null);
    setDraftText("");
    setEditingCommentId(null);
  };

  const buildReviewSubmission = () => {
    const commentLines = activeComments.map((comment) => `- L${comment.line}: ${comment.text}`);
    const manual = generalReviewText.trim();
    return [
      manual ? `General feedback:\n${manual}` : "",
      commentLines.length > 0 ? `Line comments:\n${commentLines.join("\n")}` : "",
    ]
      .filter(Boolean)
      .join("\n\n");
  };

  return {
    markdownArtifacts,
    selectedArtifactPath,
    setSelectedArtifactPath,
    previewText,
    previewLines,
    activeComments,
    draftLine,
    draftText,
    setDraftText,
    draftAnchorTop,
    draftTextareaRef,
    reviewViewportRef,
    lineButtonRefs,
    generalReviewText,
    setGeneralReviewText,
    showGeneralReviewInput,
    setShowGeneralReviewInput,
    openCommentEditor,
    saveComment,
    deleteComment,
    startEditingComment,
    cancelDraft,
    buildReviewSubmission,
  };
}
