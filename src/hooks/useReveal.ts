import { useEffect, useRef, useState } from "react";

/**
 * useReveal — attaches an IntersectionObserver to a ref and returns whether
 * the element has entered the viewport. Once revealed it stays revealed.
 *
 * Usage:
 *   const { ref, revealed } = useReveal();
 *   <div ref={ref} className={`reveal ${revealed ? "revealed" : ""}`} />
 */
export function useReveal(threshold = 0.15) {
  const ref = useRef<HTMLElement | null>(null);
  const [revealed, setRevealed] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setRevealed(true);
          observer.disconnect();
        }
      },
      { threshold }
    );

    observer.observe(el);
    return () => observer.disconnect();
  }, [threshold]);

  return { ref, revealed };
}

/**
 * useRevealGroup — returns a single ref and a revealed state, meant to be
 * placed on a container so that children can stagger via CSS delay classes.
 */
export function useRevealGroup(threshold = 0.1) {
  return useReveal(threshold);
}
