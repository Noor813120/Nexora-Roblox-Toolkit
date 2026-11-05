import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => {
  cleanup();
  window.location.hash = "";
  delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});
