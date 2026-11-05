import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useAction } from "./useAction";

describe("useAction", () => {
  it("goes idle → pending → done and returns the result", async () => {
    let finish!: (v: number) => void;
    const { result } = renderHook(() => useAction(() => new Promise<number>((r) => (finish = r))));
    expect(result.current.state.status).toBe("idle");
    let promise!: Promise<number | undefined>;
    act(() => {
      promise = result.current.run();
    });
    expect(result.current.pending).toBe(true);
    await act(async () => {
      finish(42);
      await promise;
    });
    expect(await promise).toBe(42);
    expect(result.current.state.status).toBe("done");
  });

  it("returns undefined and keeps a readable message when the call fails", async () => {
    const { result } = renderHook(() =>
      useAction(async () => {
        throw { code: "conflict", message: "name taken" };
      }),
    );
    let value: unknown = "unset";
    await act(async () => {
      value = await result.current.run();
    });
    expect(value).toBeUndefined();
    expect(result.current.state).toEqual({ status: "error", message: "name taken" });
    act(() => result.current.reset());
    expect(result.current.state.status).toBe("idle");
  });

  it("ignores a second call while the first is still running", async () => {
    let finish!: () => void;
    const action = vi.fn(() => new Promise<void>((r) => (finish = r)));
    const { result } = renderHook(() => useAction(action));
    act(() => {
      void result.current.run();
    });
    let second: unknown = "unset";
    await act(async () => {
      second = await result.current.run();
    });
    expect(second).toBeUndefined();
    expect(action).toHaveBeenCalledTimes(1);
    await act(async () => finish());
    await waitFor(() => expect(result.current.state.status).toBe("done"));
    // And it can run again once finished.
    act(() => {
      void result.current.run();
    });
    expect(action).toHaveBeenCalledTimes(2);
    await act(async () => finish());
  });

  it("does not touch state after unmount", async () => {
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    let finish!: () => void;
    const { result, unmount } = renderHook(() => useAction(() => new Promise<void>((r) => (finish = r))));
    act(() => {
      void result.current.run();
    });
    unmount();
    await act(async () => finish());
    expect(errors).not.toHaveBeenCalled();
    errors.mockRestore();
  });
});
