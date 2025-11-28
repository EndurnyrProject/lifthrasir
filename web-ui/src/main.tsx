import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/fonts.css";
import "./styles/typography.css";
import { invoke } from "@tauri-apps/api/core";

invoke("refresh_window").catch(console.error);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
