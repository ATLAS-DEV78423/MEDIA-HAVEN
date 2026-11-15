import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { HashRouter } from "react-router-dom";
import { App } from "./App";
import "./styles/tokens.css";
import "./styles/global.css";
import "./styles/motion.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("Root container #root is missing from index.html");
}

createRoot(container).render(
  <StrictMode>
    {/*
      HashRouter for now: the Tauri host serves the frontend from a custom
      protocol, where path-based routing needs a server-side rewrite that
      does not exist yet. This changes to BrowserRouter with the shell.
    */}
    <HashRouter>
      <App />
    </HashRouter>
  </StrictMode>,
);
