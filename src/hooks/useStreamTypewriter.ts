import { useEffect, useRef, useState } from "react";

const CHARS_PER_FRAME = 4;
const FRAME_MS = 8;

export function useStreamTypewriter(content: string, isStreaming: boolean) {
  const [displayedContent, setDisplayedContent] = useState(content);
  const contentRef = useRef(content);
  const pendingRef = useRef("");
  const frameRef = useRef<number | null>(null);

  useEffect(() => {
    if (!isStreaming) {
      setDisplayedContent(content);
      contentRef.current = content;
      pendingRef.current = "";
      if (frameRef.current) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }
      return;
    }

    const newContent = content.slice(contentRef.current.length);
    pendingRef.current += newContent;
    contentRef.current = content;

    if (frameRef.current) return;

    const typeNext = () => {
      if (pendingRef.current.length === 0) {
        frameRef.current = null;
        return;
      }

      const charsToAdd = pendingRef.current.slice(0, CHARS_PER_FRAME);
      pendingRef.current = pendingRef.current.slice(CHARS_PER_FRAME);

      setDisplayedContent((prev) => prev + charsToAdd);

      frameRef.current = requestAnimationFrame(() => {
        setTimeout(typeNext, FRAME_MS);
      });
    };

    frameRef.current = requestAnimationFrame(() => {
      setTimeout(typeNext, FRAME_MS);
    });

    return () => {
      if (frameRef.current) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }
    };
  }, [content, isStreaming]);

  return displayedContent;
}
