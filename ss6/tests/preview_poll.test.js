import { describe, it, expect } from "vitest";
import { collectVisibleIds, buildDetailFromRows } from "./pure.js";

describe("collectVisibleIds", () => {
  it("returns empty for empty bindings", () => {
    expect(collectVisibleIds([])).toEqual([]);
  });

  it("returns only visible non-text regs", () => {
    const bindings = [
      { visible: true, is_text: false, reg_id: 1 },
      { visible: true, is_text: false, reg_id: 2 },
      { visible: false, is_text: false, reg_id: 3 },
      { visible: true, is_text: true, reg_id: 4 },
    ];
    expect(collectVisibleIds(bindings)).toEqual([1, 2]);
  });

  it("deduplicates reg_ids", () => {
    const bindings = [
      { visible: true, is_text: false, reg_id: 1 },
      { visible: true, is_text: false, reg_id: 1 },
    ];
    expect(collectVisibleIds(bindings)).toEqual([1]);
  });

  it("filters out reg_id <= 0", () => {
    const bindings = [
      { visible: true, is_text: false, reg_id: 0 },
      { visible: true, is_text: false, reg_id: -1 },
    ];
    expect(collectVisibleIds(bindings)).toEqual([]);
  });

  it("preserves insertion order", () => {
    const bindings = [
      { visible: true, is_text: false, reg_id: 3 },
      { visible: true, is_text: false, reg_id: 1 },
      { visible: true, is_text: false, reg_id: 2 },
    ];
    expect(collectVisibleIds(bindings)).toEqual([3, 1, 2]);
  });
});

describe("buildDetailFromRows", () => {
  const valueForStatus = (row) => String(row.val_num ?? row.val_txt ?? "-");

  it("returns detail with count for empty rows", () => {
    const result = buildDetailFromRows([], valueForStatus, () => "");
    expect(result.detail).toBe("0 regs");
    expect(result.debug).toBe("");
  });

  it("builds debug with values", () => {
    const rows = [
      { reg_id: 1, val_num: 42 },
      { reg_id: 2, val_num: 100 },
    ];
    const result = buildDetailFromRows(rows, valueForStatus, () => "");
    expect(result.detail).toBe("2 regs");
    expect(result.debug).toBe("Values: 1=42, 2=100");
  });

  it("truncates to maxPreview and appends ellipsis", () => {
    const rows = Array.from({ length: 15 }, (_, i) => ({ reg_id: i + 1, val_num: i }));
    const result = buildDetailFromRows(rows, valueForStatus, () => "", 5);
    expect(result.debug).toContain(", ...");
    const parts = result.debug.match(/\d+=\d+/g);
    expect(parts.length).toBe(5);
  });

  it("handles cleanHex for rows with req_hex", () => {
    const rows = [{ reg_id: 1, val_num: 42, req_hex: "01 02" }];
    const cleanHex = (h) => h.toUpperCase();
    const result = buildDetailFromRows(rows, valueForStatus, cleanHex);
    expect(result.detail).toBe("1 regs");
  });
});
