import { describe, it, expect } from "vitest";
import { sourceTone } from "./pure.js";

describe("sourceTone", () => {
  it("returns good for active sources", () => {
    expect(sourceTone("real")).toBe("good");
    expect(sourceTone("db")).toBe("good");
    expect(sourceTone("write")).toBe("good");
    expect(sourceTone("ready")).toBe("good");
  });

  it("returns warn for pending", () => {
    expect(sourceTone("pending")).toBe("warn");
  });

  it("returns danger for error", () => {
    expect(sourceTone("error")).toBe("danger");
  });

  it("returns muted for unknown", () => {
    expect(sourceTone("idle")).toBe("muted");
    expect(sourceTone("")).toBe("muted");
    expect(sourceTone(null)).toBe("muted");
  });
});
