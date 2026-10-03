import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { CaretApp } from "./CaretApp";
import "../shared/base.css";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <CaretApp />
  </StrictMode>,
);
