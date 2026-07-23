import { describe, expect, it } from "vitest";
import { greet } from "./greeting.js";

describe("greet", () => {
  it("returns a friendly greeting", () => {
    expect(greet("Alice")).toBe("Hello, Alice!");
  });

  it("trims leading and trailing whitespace", () => {
    expect(greet("  Bob  ")).toBe("Hello, Bob!");
  });

  it("rejects an empty name", () => {
    expect(() => greet("")).toThrow();
  });

  it("rejects a whitespace-only name", () => {
    expect(() => greet("   ")).toThrow();
  });
});
