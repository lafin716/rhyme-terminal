import { describe, expect, it } from "vitest";
import { sessionIndicatorClass } from "./session-indicator";

describe("sessionIndicatorClass", () => {
  it("uses the ready indicator when a terminal has no agent task status", () => {
    expect(sessionIndicatorClass()).toBe("is-idle");
  });

  it.each([
    ["working", "is-working"],
    ["completed", "is-completed"],
    ["error", "is-error"],
  ] as const)("preserves the %s status class", (status, expected) => {
    expect(sessionIndicatorClass(status)).toBe(expected);
  });
});
