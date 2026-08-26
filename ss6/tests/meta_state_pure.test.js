import { describe, it, expect } from "vitest";
import { clampScale, buildDetailFromRows } from "./pure.js";

describe("clampScale", () => {
  it("returns 1 for null/undefined", () => {
    expect(clampScale(null)).toBe(1);
    expect(clampScale(undefined)).toBe(1);
  });

  it("clamps to min 0.25", () => {
    expect(clampScale(0.1)).toBe(0.25);
    expect(clampScale(-1)).toBe(0.25);
  });

  it("treats 0 as default (1)", () => {
    expect(clampScale(0)).toBe(1);
  });

  it("clamps to max 4", () => {
    expect(clampScale(5)).toBe(4);
    expect(clampScale(10)).toBe(4);
  });

  it("passes through valid values", () => {
    expect(clampScale(0.5)).toBe(0.5);
    expect(clampScale(1)).toBe(1);
    expect(clampScale(2)).toBe(2);
    expect(clampScale(3.5)).toBe(3.5);
  });

  it("handles string input", () => {
    expect(clampScale("2")).toBe(2);
    expect(clampScale("0.5")).toBe(0.5);
  });
});
