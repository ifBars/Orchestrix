import { useState, useCallback, useRef, useEffect } from "react";

export type HoverState = {
  line: number;
  element: HTMLElement;
} | null;

export function useCommentHover(
  proseRef: React.RefObject<HTMLDivElement | null>,
  buttonRef: React.RefObject<HTMLButtonElement | null>,
  draftLine: number | null,
  previewText: string
) {
  const [hoverState, setHoverState] = useState<HoverState>(null);
  const hideTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  // Assign line numbers and hover handlers to block-level elements after render
  useEffect(() => {
    if (!proseRef.current) return;

    const blocks = proseRef.current.querySelectorAll(
      "p, h1, h2, h3, h4, h5, h6, li, pre, blockquote, table, hr"
    );

    const handleMouseEnter = (e: Event) => {
      if (draftLine != null) return;
      clearHoverTimeout();
      const block = e.currentTarget as HTMLElement;
      const line = parseInt(block.dataset.line || "1", 10);
      setHoverState({ line, element: block });
    };

    const handleMouseLeave = (e: MouseEvent) => {
      const relatedTarget = e.relatedTarget as HTMLElement | null;
      // Don't hide if moving to the comment button
      if (
        buttonRef.current &&
        (buttonRef.current === relatedTarget ||
          buttonRef.current.contains(relatedTarget))
      ) {
        return;
      }
      // Don't hide if moving to another block (the new block's mouseenter will handle it)
      if (relatedTarget && proseRef.current?.contains(relatedTarget)) {
        return;
      }
      scheduleHide();
    };

    blocks.forEach((block, index) => {
      const el = block as HTMLElement;
      el.dataset.line = String(index + 1);
      el.addEventListener("mouseenter", handleMouseEnter);
      el.addEventListener("mouseleave", handleMouseLeave as EventListener);
    });

    return () => {
      blocks.forEach((block) => {
        const el = block as HTMLElement;
        el.removeEventListener("mouseenter", handleMouseEnter);
        el.removeEventListener("mouseleave", handleMouseLeave as EventListener);
      });
    };
  }, [previewText, draftLine]);

  const clearHoverTimeout = useCallback(() => {
    if (hideTimeoutRef.current) {
      clearTimeout(hideTimeoutRef.current);
      hideTimeoutRef.current = null;
    }
  }, []);

  const scheduleHide = useCallback(() => {
    clearHoverTimeout();
    hideTimeoutRef.current = setTimeout(() => {
      setHoverState(null);
    }, 150);
  }, [clearHoverTimeout]);

  const handleButtonMouseEnter = useCallback(() => {
    clearHoverTimeout();
  }, [clearHoverTimeout]);

  const handleButtonMouseLeave = useCallback(() => {
    scheduleHide();
  }, [scheduleHide]);

  // Calculate button position relative to the hovered element
  const getButtonStyle = useCallback(() => {
    if (!hoverState || !proseRef.current) return {};

    const block = hoverState.element;
    const containerRect = proseRef.current.getBoundingClientRect();

    // Find the actual content width by getting the range of the text content
    const range = document.createRange();
    const textNodes: Node[] = [];

    // Collect all text and inline nodes
    const walker = document.createTreeWalker(
      block,
      NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT,
      {
        acceptNode: (node) => {
          if (node.nodeType === Node.TEXT_NODE) {
            return node.textContent?.trim()
              ? NodeFilter.FILTER_ACCEPT
              : NodeFilter.FILTER_REJECT;
          }
          const el = node as HTMLElement;
          const display = window.getComputedStyle(el).display;
          // Accept inline and inline-block elements
          if (display === "inline" || display === "inline-block") {
            return NodeFilter.FILTER_ACCEPT;
          }
          return NodeFilter.FILTER_SKIP;
        },
      }
    );

    let node;
    while ((node = walker.nextNode())) {
      textNodes.push(node);
    }

    let contentRight: number;

    if (textNodes.length > 0) {
      // Get the bounding rect of the last text node
      const lastNode = textNodes[textNodes.length - 1];
      range.selectNode(lastNode);
      const rect = range.getBoundingClientRect();
      contentRight = rect.right;
    } else {
      // Fallback to block rect if no text found
      contentRight = block.getBoundingClientRect().right;
    }

    const top = block.getBoundingClientRect().top - containerRect.top + 4;
    const left = contentRight - containerRect.left + 8;

    return {
      top: Math.max(4, top),
      left: Math.max(8, left),
    };
  }, [hoverState]);

  return {
    hoverState,
    setHoverState,
    clearHoverTimeout,
    scheduleHide,
    handleButtonMouseEnter,
    handleButtonMouseLeave,
    getButtonStyle,
  };
}
