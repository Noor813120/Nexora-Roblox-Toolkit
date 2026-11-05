import { describe, expect, it } from "vitest";
import { describeError } from "./errors";

describe("describeError", () => {
  it("uses the message of a backend AppError", () => {
    expect(describeError({ code: "invalid", message: "invalid request: name is empty" })).toBe(
      "invalid request: name is empty",
    );
  });

  it("passes a plain string through", () => {
    expect(describeError("command not found")).toBe("command not found");
  });

  it("uses an Error's message", () => {
    expect(describeError(new Error("boom"))).toBe("boom");
  });

  it("falls back to a generic message instead of [object Object]", () => {
    for (const odd of [null, undefined, 42, {}, { message: 7 }, "  ", { message: "" }]) {
      expect(describeError(odd)).toBe("Something went wrong.");
    }
  });
});
