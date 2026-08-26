import { describe, it, expect } from "vitest";
import {
  evalLevel,
  colorByLevel,
  boolState,
  alarmMarkerDefs,
  resolvePreviewRendererKey,
  buildTrendPolylinePoints,
} from "./pure.js";

describe("evalLevel", () => {
  it("returns unknown for null/undefined/empty", () => {
    expect(evalLevel(null, [])).toBe("unknown");
    expect(evalLevel(undefined, [])).toBe("unknown");
    expect(evalLevel(5, null)).toBe("unknown");
    expect(evalLevel(5, [])).toBe("unknown");
  });

  it("returns red when value <= set_lo", () => {
    expect(evalLevel(5, [{ set_lo: 10, set_hi: null, set_lo_1: null, set_hi_1: null }])).toBe("red");
    expect(evalLevel(10, [{ set_lo: 10, set_hi: null, set_lo_1: null, set_hi_1: null }])).toBe("red");
  });

  it("returns red when value >= set_hi", () => {
    expect(evalLevel(100, [{ set_lo: null, set_hi: 90, set_lo_1: null, set_hi_1: null }])).toBe("red");
    expect(evalLevel(90, [{ set_lo: null, set_hi: 90, set_lo_1: null, set_hi_1: null }])).toBe("red");
  });

  it("returns green when value is in normal range", () => {
    expect(evalLevel(50, [{ set_lo: 10, set_hi: 90, set_lo_1: null, set_hi_1: null }])).toBe("green");
  });

  it("returns yellow for warning range (lo_1)", () => {
    expect(evalLevel(15, [{ set_lo: 10, set_hi: 90, set_lo_1: 20, set_hi_1: null }])).toBe("yellow");
  });

  it("returns yellow for warning range (hi_1)", () => {
    expect(evalLevel(85, [{ set_lo: 10, set_hi: 90, set_lo_1: null, set_hi_1: 80 }])).toBe("yellow");
  });

  it("returns yellow for lo_1 without set_lo", () => {
    expect(evalLevel(5, [{ set_lo: null, set_hi: null, set_lo_1: 10, set_hi_1: null }])).toBe("yellow");
  });

  it("returns yellow for hi_1 without set_hi", () => {
    expect(evalLevel(95, [{ set_lo: null, set_hi: null, set_lo_1: null, set_hi_1: 90 }])).toBe("yellow");
  });
});

describe("colorByLevel", () => {
  it("maps red to red color", () => expect(colorByLevel("red")).toBe("#d23939"));
  it("maps yellow to yellow color", () => expect(colorByLevel("yellow")).toBe("#e2c33b"));
  it("maps green to green color", () => expect(colorByLevel("green")).toBe("#2abf62"));
  it("maps unknown to default", () => expect(colorByLevel("unknown")).toBe("#3a58a4"));
  it("maps anything else to default", () => expect(colorByLevel("foo")).toBe("#3a58a4"));
});

describe("boolState", () => {
  it("returns null for null/undefined/NaN", () => {
    expect(boolState(null, 0)).toBe(null);
    expect(boolState(undefined, 0)).toBe(null);
    expect(boolState(NaN, 0)).toBe(null);
  });

  it("treats small values as boolean", () => {
    expect(boolState(0, 0)).toBe(false);
    expect(boolState(1, 0)).toBe(true);
    expect(boolState(0.5, 0)).toBe(true);
    expect(boolState(0.4, 0)).toBe(false);
  });

  it("extracts bit for larger integers", () => {
    expect(boolState(255, 0)).toBe(true);
    expect(boolState(255, 7)).toBe(true);
    expect(boolState(254, 0)).toBe(false);
    expect(boolState(1, 1)).toBe(true);
    expect(boolState(2, 1)).toBe(true);
  });

  it("handles negative values", () => {
    expect(boolState(-1, 0)).toBe(false);
    expect(boolState(-0.5, 0)).toBe(false);
  });
});

describe("alarmMarkerDefs", () => {
  it("returns empty for null/empty", () => {
    expect(alarmMarkerDefs(null)).toEqual([]);
    expect(alarmMarkerDefs([])).toEqual([]);
  });

  it("extracts finite values with correct colors", () => {
    const rules = [{ set_lo: 10, set_hi: 90, set_lo_1: 20, set_hi_1: 80, enabled: true }];
    const markers = alarmMarkerDefs(rules);
    expect(markers).toEqual([
      { value: 10, color: "#d23939" },
      { value: 90, color: "#d23939" },
      { value: 20, color: "#e2c33b" },
      { value: 80, color: "#e2c33b" },
    ]);
  });

  it("skips disabled rules", () => {
    const rules = [{ set_lo: 10, set_hi: 90, set_lo_1: null, set_hi_1: null, enabled: false }];
    expect(alarmMarkerDefs(rules)).toEqual([]);
  });

  it("skips null values (Number(null) is 0)", () => {
    const rules = [{ set_lo: null, set_hi: 90, set_lo_1: null, set_hi_1: null, enabled: true }];
    const markers = alarmMarkerDefs(rules);
    expect(markers.length).toBe(4);
    expect(markers[1]).toEqual({ value: 90, color: "#d23939" });
  });
});

describe("resolvePreviewRendererKey", () => {
  it("returns image for text+image", () => {
    expect(resolvePreviewRendererKey("image", { isText: true, isTu: false, isBool: false })).toBe("image");
  });

  it("returns text for text", () => {
    expect(resolvePreviewRendererKey("auto", { isText: true, isTu: false, isBool: false })).toBe("text");
  });

  it("returns button", () => {
    expect(resolvePreviewRendererKey("button", { isText: false, isTu: false, isBool: false })).toBe("button");
  });

  it("returns led for led kind", () => {
    expect(resolvePreviewRendererKey("led", { isText: false, isTu: false, isBool: false })).toBe("led");
  });

  it("returns led for auto+bool", () => {
    expect(resolvePreviewRendererKey("auto", { isText: false, isTu: false, isBool: true })).toBe("led");
  });

  it("returns tu for tu binding", () => {
    expect(resolvePreviewRendererKey("auto", { isText: false, isTu: true, isBool: false })).toBe("tu");
  });

  it("returns bar", () => {
    expect(resolvePreviewRendererKey("bar", { isText: false, isTu: false, isBool: false })).toBe("bar");
  });

  it("returns gauge", () => {
    expect(resolvePreviewRendererKey("gauge", { isText: false, isTu: false, isBool: false })).toBe("gauge");
  });

  it("returns setpoint", () => {
    expect(resolvePreviewRendererKey("setpoint", { isText: false, isTu: false, isBool: false })).toBe("setpoint");
  });

  it("returns numeric for numeric kind", () => {
    expect(resolvePreviewRendererKey("numeric", { isText: false, isTu: false, isBool: false })).toBe("numeric");
  });

  it("returns numeric for auto kind (non-bool)", () => {
    expect(resolvePreviewRendererKey("auto", { isText: false, isTu: false, isBool: false })).toBe("numeric");
  });

  it("returns trend", () => {
    expect(resolvePreviewRendererKey("trend", { isText: false, isTu: false, isBool: false })).toBe("trend");
  });

  it("returns fallback for unknown kind", () => {
    expect(resolvePreviewRendererKey("foo", { isText: false, isTu: false, isBool: false })).toBe("fallback");
  });
});

describe("buildTrendPolylinePoints", () => {
  it("returns empty for < 2 samples", () => {
    expect(buildTrendPolylinePoints([], 100, 4, 92, 4, 30)).toBe("");
    expect(buildTrendPolylinePoints([50], 100, 4, 92, 4, 30)).toBe("");
  });

  it("builds correct points for 2 samples", () => {
    const result = buildTrendPolylinePoints([0, 100], 100, 4, 92, 4, 30);
    expect(result).toContain("4.00");
    expect(result).toContain("96.00");
  });

  it("normalizes values by scaleMax", () => {
    const result = buildTrendPolylinePoints([50, 50], 100, 4, 92, 4, 30);
    const points = result.split(" ");
    expect(points.length).toBe(2);
    const [, y1] = points[0].split(",");
    const y = Number(y1);
    expect(y).toBeGreaterThan(4);
    expect(y).toBeLessThan(34);
  });

  it("clamps values to 0..1 range", () => {
    const result = buildTrendPolylinePoints([200, -10], 100, 4, 92, 4, 30);
    const points = result.split(" ");
    expect(points.length).toBe(2);
  });
});
