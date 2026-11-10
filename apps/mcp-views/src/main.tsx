import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@modelcontextprotocol/ext-apps";
import { config } from "zod/v4";
import { createStore } from "./store";
import { StatsView } from "./stats";
import "./styles.css";

// zod probes for `new Function` before it compiles a schema; the view's policy forbids eval, so
// the probe would only log a refused script.
config({ jitless: true });

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

// The view's own version, not Kaeso's: the bundle is committed, and a release must not change it.
const app = new App({ name: "Kaeso usage stats", version: "1.0.0" }, { availableDisplayModes: ["inline", "fullscreen"] });
const store = createStore(app);
createRoot(root).render(
  <StrictMode>
    <StatsView store={store} />
  </StrictMode>,
);
app.connect().then(store.connected, store.failed);
