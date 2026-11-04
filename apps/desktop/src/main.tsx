import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { loadSlotFonts } from "./lib/fonts";
import { Panel } from "./views/Panel";
import { Permissions } from "./views/Permissions";
import "./styles.css";

// One bundle, one view per window label.
const label = getCurrentWindow().label;
document.documentElement.dataset.view = label;
loadSlotFonts();

createRoot(document.getElementById("root")!).render(
  <StrictMode>{label === "panel" ? <Panel /> : <Permissions />}</StrictMode>,
);
