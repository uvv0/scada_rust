import { describe, it, expect } from "vitest";
import { cleanHex } from "./pure.js";

describe("cleanHex", () => {
  it("returns empty string for falsy input", () => {
    expect(cleanHex("")).toBe("");
    expect(cleanHex(null)).toBe("");
    expect(cleanHex(undefined)).toBe("");
  });

  it("extracts hex pairs from string", () => {
    expect(cleanHex("0A1B2C")).toBe("0A 1B 2C");
    expect(cleanHex("hello")).toBe("");
    expect(cleanHex("a1b2")).toBe("A1 B2");
  });

  it("handles mixed content", () => {
    expect(cleanHex("AA BB CC")).toBe("AA BB CC");
    expect(cleanHex("0A1B2C3D")).toBe("0A 1B 2C 3D");
  });

  it("ignores odd characters", () => {
    expect(cleanHex("xyz")).toBe("");
    expect(cleanHex("G1")).toBe("");
  });
});
