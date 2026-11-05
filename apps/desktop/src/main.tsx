import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { installPreview } from "./preview";
import { loadSlotFonts } from "./lib/fonts";
import { loadTheme } from "./lib/theme";
import { installTips } from "./lib/tips";
import { Panel } from "./views/Panel";
import { Overlay } from "./views/Overlay";
import { Shelf } from "./views/Shelf";
import { Settings } from "./views/Settings";
import { NoteWindow } from "./views/NoteWindow";
import { Board } from "./views/Board";
import { Guide } from "./views/Guide";
import "./styles/base.css";
import "./styles/panel.css";
import "./styles/overlay.css";
import "./styles/shelf.css";
import "./styles/paper.css";
import "./styles/windows.css";
import "./styles/board.css";
import "./styles/pixel.css";

// One bundle, one view per window label.
installPreview();
const label = getCurrentWindow().label;
document.documentElement.dataset.view = label;
loadTheme();
loadSlotFonts();
// The buddy explains things you hover in Pixie's own windows.
if (["panel", "settings", "note", "guide"].includes(label) && !new URLSearchParams(location.search).get("preview")) installTips();

const VIEWS: Record<string, () => React.JSX.Element> = {
  panel: Panel,
  overlay: Overlay,
  shelf: Shelf,
  settings: Settings,
  note: NoteWindow,
  board: Board,
  guide: Guide,
};
const View = VIEWS[label] ?? Settings;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <View />
  </StrictMode>,
);
