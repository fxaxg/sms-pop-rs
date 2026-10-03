import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { ToastApp } from "./ToastApp";
import "../shared/base.css";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ToastApp />
  </StrictMode>,
);
