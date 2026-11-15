import { StrictMode, Suspense, lazy } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { installPreview } from "./preview";
import { loadSlotFonts } from "./lib/fonts";
import { loadTheme } from "./lib/theme";
import { installTips } from "./lib/tips";
import { Panel } from "./views/Panel";
import { Overlay } from "./views/Overlay";
import { Shelf } from "./views/Shelf";
import { NoteWindow } from "./views/NoteWindow";
import { Guide } from "./views/Guide";
import "./styles/base.css";
import "./styles/panel.css";
import "./styles/overlay.css";
import "./styles/shelf.css";
import "./styles/paper.css";
import "./styles/windows.css";
import "./styles/pixel.css";

// One bundle, one view per window label.
installPreview();
const label = getCurrentWindow().label;
document.documentElement.dataset.view = label;
loadTheme();
loadSlotFonts();
// The buddy explains things you hover in Pixie's own windows.
if (["panel", "settings", "note", "guide"].includes(label) && !new URLSearchParams(location.search).get("preview")) installTips();

// Excalidraw (experimental canvas) is big: only its window loads it. Fonts are served locally.
(window as unknown as { EXCALIDRAW_ASSET_PATH: string }).EXCALIDRAW_ASSET_PATH = "/excalidraw-assets/";
const LazyCanvas = lazy(() => import("./views/Canvas").then((m) => ({ default: m.Canvas })));
const CanvasView = () => (
  <Suspense fallback={<div className="canvas-empty">Opening the canvas…</div>}>
    <LazyCanvas />
  </Suspense>
);

// Three.js only loads in the 3D window.
const LazyScene = lazy(() => import("./views/Scene3D").then((m) => ({ default: m.Scene3D })));
const SceneView = () => (
  <Suspense fallback={<div className="canvas-empty">Opening the 3D view…</div>}>
    <LazyScene />
  </Suspense>
);

// Settings is the biggest view: only its own window loads it, so the notch, the buddy and the
// shelf (always running) start quicker and stay lighter.
const LazySettings = lazy(() => import("./views/Settings").then((m) => ({ default: m.Settings })));
const Settings = () => (
  <Suspense fallback={null}>
    <LazySettings />
  </Suspense>
);

const VIEWS: Record<string, () => React.JSX.Element> = {
  panel: Panel,
  overlay: Overlay,
  shelf: Shelf,
  settings: Settings,
  note: NoteWindow,
  guide: Guide,
  canvas: CanvasView,
  scene: SceneView,
};
const View = VIEWS[label] ?? Settings;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <View />
  </StrictMode>,
);
