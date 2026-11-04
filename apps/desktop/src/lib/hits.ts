import { useEffect } from "react";
import { api, type Rect } from "./bridge";

/** Report the on-screen rectangles of the given elements so the rest of a transparent window
 *  lets clicks through to the apps underneath. */
export function useHitRects(label: string, refs: React.RefObject<HTMLElement | null>[], deps: unknown[]) {
  useEffect(() => {
    let raf = 0;
    const send = () => {
      const rects: Rect[] = refs
        .map((r) => r.current?.getBoundingClientRect())
        .filter((b): b is DOMRect => !!b && b.width > 0 && b.height > 0)
        .map((b) => ({ x: b.left, y: b.top, w: b.width, h: b.height }));
      api.hitRects(label, rects).catch(() => {});
    };
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(send);
    });
    refs.forEach((r) => r.current && ro.observe(r.current));
    send();
    // Shapes animate; re-send after the spring settles.
    const t = setTimeout(send, 420);
    return () => {
      ro.disconnect();
      cancelAnimationFrame(raf);
      clearTimeout(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
