// "Drag me into the list above": a card under System Settings with a real, draggable app
// tile (native file drag), shown while a Privacy permission is being switched on.
import { useEffect, useState } from "react";
import { startDrag } from "@crabnebula/tauri-plugin-drag";
import { api, onEvent } from "../lib/bridge";
import { Glyph, Mark } from "../components/Mark";

const NAMES: Record<string, string> = { accessibility: "Accessibility", screen: "Screen & System Audio Recording" };

export function Guide() {
  const [perm, setPerm] = useState("");
  const [app, setApp] = useState("");
  const [icon, setIcon] = useState("");
  const [done, setDone] = useState(false);

  useEffect(() => {
    const un = onEvent((e) => {
      if (e.kind === "guide.show") {
        setPerm(e.payload.perm);
        setApp(e.payload.app);
        setIcon(e.payload.icon);
        setDone(false);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  // Close by itself once the switch is on.
  useEffect(() => {
    if (!perm) return;
    const t = setInterval(async () => {
      const list = await api.permissions().catch(() => []);
      if (list.find((p) => p.id === perm)?.state === "granted") {
        setDone(true);
        api.playSound("tada").catch(() => {});
        setTimeout(() => api.guideHide(), 1400);
        clearInterval(t);
      }
    }, 1500);
    return () => clearInterval(t);
  }, [perm]);

  return (
    <div className="guide">
      <div className="guide__card">
        <div className="guide__head">
          <span className="guide__arrow"><Glyph name="arrow" size={16} /></span>
          <span className="guide__text">
            {done ? "Got it. Thank you!" : `I'm Pixie. Drag me into the ${NAMES[perm] ?? ""} list above.`}
          </span>
          <button className="guide__x" onClick={() => api.guideHide()} aria-label="Close">×</button>
        </div>
        {!done && (
          <div
            className="guide__tile"
            onMouseDown={(e) => {
              e.preventDefault();
              startDrag({ item: [app], icon }).catch(console.error);
            }}
            title="Drag into the list in System Settings"
          >
            <Mark size={34} />
            <span className="guide__name">Pixie</span>
            <span className="guide__hint">drag ↑</span>
          </div>
        )}
      </div>
    </div>
  );
}
