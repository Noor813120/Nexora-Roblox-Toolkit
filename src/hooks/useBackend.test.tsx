import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useBackend } from "./useBackend";

const enterTauri = () => {
  (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
};

describe("useBackend", () => {
  it("is offline, and never calls the loader, outside the app window", async () => {
    const load = vi.fn(async () => 1);
    const { result } = renderHook(() => useBackend(load));
    await waitFor(() => expect(result.current.state.status).toBe("offline"));
    expect(load).not.toHaveBeenCalled();
  });

  it("goes from loading to ready with the data and a timestamp", async () => {
    enterTauri();
    const { result } = renderHook(() => useBackend(async () => "hello"));
    expect(result.current.state.status).toBe("loading");
    await waitFor(() => expect(result.current.state.status).toBe("ready"));
    const s = result.current.state;
    expect(s.status === "ready" && s.data).toBe("hello");
    expect(s.status === "ready" && s.at).toBeGreaterThan(0);
  });

  it("reports a backend error as readable text", async () => {
    enterTauri();
    const { result } = renderHook(() =>
      useBackend(async () => {
        throw { code: "internal", message: "internal error" };
      }),
    );
    await waitFor(() => expect(result.current.state).toEqual({ status: "error", message: "internal error" }));
  });

  it("loads once on mount and again only when reload is called (no polling)", async () => {
    enterTauri();
    const load = vi.fn(async () => 1);
    const { result } = renderHook(() => useBackend(load));
    await waitFor(() => expect(result.current.state.status).toBe("ready"));
    await new Promise((r) => setTimeout(r, 60));
    expect(load).toHaveBeenCalledTimes(1);
    act(() => result.current.reload());
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  });

  it("keeps showing the old data while a reload is in flight", async () => {
    enterTauri();
    const pending: Array<(v: number) => void> = [];
    const load = () => new Promise<number>((resolve) => pending.push(resolve));
    const { result } = renderHook(() => useBackend(load));
    await act(async () => pending[0]?.(1));
    expect(result.current.state.status === "ready" && result.current.state.data).toBe(1);
    act(() => result.current.reload());
    expect(result.current.state.status).toBe("ready");
    await act(async () => pending[1]?.(2));
    expect(result.current.state.status === "ready" && result.current.state.data).toBe(2);
  });

  it("drops a response that arrives after a newer request", async () => {
    enterTauri();
    const pending: Array<(v: number) => void> = [];
    const load = () => new Promise<number>((resolve) => pending.push(resolve));
    const { result } = renderHook(() => useBackend(load));
    act(() => result.current.reload()); // second request while the first is pending
    await act(async () => pending[1]?.(2)); // newest answers first
    await act(async () => pending[0]?.(1)); // stale answer arrives late
    expect(result.current.state.status === "ready" && result.current.state.data).toBe(2);
  });
});
