import { useRef, useLayoutEffect } from "react";

function updateTitlebarBounds(
  leftEl: HTMLElement | null,
  rightEl: HTMLElement | null,
) {
  const titlebar = document.getElementById("titlebar");
  if (!titlebar) return;

  const leftBound = leftEl ? leftEl.getBoundingClientRect().right : 80;
  const rightBound = rightEl
    ? rightEl.getBoundingClientRect().left
    : window.innerWidth;

  titlebar.style.left = `${leftBound}px`;
  titlebar.style.right = `${window.innerWidth - rightBound}px`;
  titlebar.style.pointerEvents = "auto";
}

/**
 * Hook to manage titlebar drag region bounds.
 * Returns refs to attach to left and right header elements.
 * The drag region will be positioned between these elements.
 */
export function useTitlebar(deps: unknown[] = []) {
  const leftRef = useRef<HTMLDivElement>(null);
  const rightRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    updateTitlebarBounds(leftRef.current, rightRef.current);

    const handleResize = () =>
      updateTitlebarBounds(leftRef.current, rightRef.current);
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  return { leftRef, rightRef };
}
