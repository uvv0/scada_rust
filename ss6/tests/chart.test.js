import { describe, it, expect } from "vitest";
import { fmtStatValue, fmtStatTime, chartTitle, buildSeries } from "./pure.js";

describe("fmtStatValue", () => {
  it("returns - for non-finite values", () => {
    expect(fmtStatValue(null)).toBe("-");
    expect(fmtStatValue(undefined)).toBe("-");
    expect(fmtStatValue(NaN)).toBe("-");
    expect(fmtStatValue(Infinity)).toBe("-");
  });

  it("formats large values without decimals", () => {
    expect(fmtStatValue(1000)).toBe("1000");
    expect(fmtStatValue(12345)).toBe("12345");
    expect(fmtStatValue(9999)).toBe("9999");
  });

  it("formats values >= 100 with 1 decimal", () => {
    expect(fmtStatValue(100)).toBe("100.0");
    expect(fmtStatValue(999.9)).toBe("999.9");
    expect(fmtStatValue(500.55)).toBe("500.6");
  });

  it("formats values < 100 with 3 decimals", () => {
    expect(fmtStatValue(0)).toBe("0.000");
    expect(fmtStatValue(99.999)).toBe("99.999");
    expect(fmtStatValue(50.5)).toBe("50.500");
  });

  it("handles negative values", () => {
    expect(fmtStatValue(-1000)).toBe("-1000");
    expect(fmtStatValue(-100)).toBe("-100.0");
    expect(fmtStatValue(-50.5)).toBe("-50.500");
  });
});

describe("fmtStatTime", () => {
  it("returns - for non-finite values", () => {
    expect(fmtStatTime(null)).toBe("-");
    expect(fmtStatTime(undefined)).toBe("-");
    expect(fmtStatTime(NaN)).toBe("-");
  });

  it("formats unix timestamp to ISO-like string", () => {
    const ts = new Date(Date.UTC(2026, 6, 15, 14, 30)).getTime() / 1000;
    const tz = new Date(ts * 1000);
    const expected = `${tz.getFullYear()}-${String(tz.getMonth() + 1).padStart(2, "0")}-${String(tz.getDate()).padStart(2, "0")} ${String(tz.getHours()).padStart(2, "0")}:${String(tz.getMinutes()).padStart(2, "0")}`;
    expect(fmtStatTime(ts)).toBe(expected);
  });
});

describe("chartTitle", () => {
  it("returns title object with given subtext", () => {
    const t = chartTitle("test subtitle");
    expect(t.text).toBe("ARX values");
    expect(t.subtext).toBe("test subtitle");
  });

  it("handles empty subtext", () => {
    const t = chartTitle("");
    expect(t.subtext).toBe("");
  });

  it("returns consistent style structure", () => {
    const t = chartTitle("x");
    expect(t).toHaveProperty("left", 10);
    expect(t).toHaveProperty("top", 8);
    expect(t.textStyle).toEqual({ color: "#eef4fb", fontSize: 18, fontWeight: 700 });
    expect(t.subtextStyle).toEqual({ color: "#8fa6bf", fontSize: 12 });
  });
});

describe("buildSeries", () => {
  it("returns empty array for null/empty rows", () => {
    expect(buildSeries(null)).toEqual([]);
    expect(buildSeries([])).toEqual([]);
    expect(buildSeries(undefined)).toEqual([]);
  });

  it("builds series with default options", () => {
    const rows = [{ reg_id: 5, points: [{ ts_unix: 1000, val_num: 50 }] }];
    const series = buildSeries(rows);
    expect(series).toHaveLength(1);
    expect(series[0].name).toBe("Reg 5");
    expect(series[0].type).toBe("line");
    expect(series[0].showSymbol).toBe(false);
    expect(series[0].smooth).toBe(false);
  });

  it("respects showSymbols and smoothLines options", () => {
    const rows = [{ reg_id: 1, points: [] }];
    const series = buildSeries(rows, true, true);
    expect(series[0].showSymbol).toBe(true);
    expect(series[0].smooth).toBe(true);
    expect(series[0].symbolSize).toBe(5);
  });

  it("converts points to ECharts format", () => {
    const rows = [
      {
        reg_id: 10,
        points: [
          { ts_unix: 1000, val_num: 42.5 },
          { ts_unix: 2000, val_num: 43.1 },
        ],
      },
    ];
    const series = buildSeries(rows);
    expect(series[0].data).toEqual([
      [1000000, 42.5],
      [2000000, 43.1],
    ]);
  });

  it("handles multiple rows", () => {
    const rows = [
      { reg_id: 1, points: [{ ts_unix: 1000, val_num: 10 }] },
      { reg_id: 2, points: [{ ts_unix: 1000, val_num: 20 }] },
    ];
    const series = buildSeries(rows);
    expect(series).toHaveLength(2);
    expect(series[0].name).toBe("Reg 1");
    expect(series[1].name).toBe("Reg 2");
  });
});
