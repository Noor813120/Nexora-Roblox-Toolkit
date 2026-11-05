import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/react";
import { afterEach } from "vitest";

// Slow or busy machines (CI, a cold first run) need more than the 1 s default to settle async UI.
configure({ asyncUtilTimeout: 4000 });

afterEach(() => {
  cleanup();
  window.location.hash = "";
  delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});
