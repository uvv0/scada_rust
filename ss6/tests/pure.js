export function cleanHex(s) {
  if (!s) return "";
  const m = String(s).toUpperCase().match(/[0-9A-F]{2}/g);
  return m ? m.join(" ") : "";
}

export function sourceTone(source) {
  if (source === "real" || source === "db" || source === "write" || source === "ready") return "good";
  if (source === "pending") return "warn";
  if (source === "error") return "danger";
  return "muted";
}

export function evalLevel(v, rules) {
  if (v === null || v === undefined || !rules || !rules.length) return "unknown";
  let yellow = false;
  for (const r of rules) {
    if (r.set_lo !== null && v <= r.set_lo) return "red";
    if (r.set_hi !== null && v >= r.set_hi) return "red";
    if (r.set_lo_1 !== null) {
      if (r.set_lo !== null) {
        if (r.set_lo_1 > r.set_lo && v > r.set_lo && v <= r.set_lo_1) yellow = true;
      } else if (v <= r.set_lo_1) yellow = true;
    }
    if (r.set_hi_1 !== null) {
      if (r.set_hi !== null) {
        if (r.set_hi_1 < r.set_hi && v >= r.set_hi_1 && v < r.set_hi) yellow = true;
      } else if (v >= r.set_hi_1) yellow = true;
    }
  }
  return yellow ? "yellow" : "green";
}

export function colorByLevel(level) {
  if (level === "red") return "#d23939";
  if (level === "yellow") return "#e2c33b";
  if (level === "green") return "#2abf62";
  return "#3a58a4";
}

export function boolState(v, bit) {
  if (v === null || v === undefined || Number.isNaN(v)) return null;
  const n = Number(v);
  if (Math.abs(n) <= 1.0) return n >= 0.5;
  if (bit !== null && bit !== undefined && bit >= 0 && bit <= 15)
    return ((Math.trunc(n) >> bit) & 1) === 1;
  return n >= 0.5;
}

export function alarmMarkerDefs(rules) {
  if (!rules || !rules.length) return [];
  const markers = [];
  for (const r of rules) {
    if (r && r.enabled === false) continue;
    const defs = [
      [r.set_lo, "#d23939"],
      [r.set_hi, "#d23939"],
      [r.set_lo_1, "#e2c33b"],
      [r.set_hi_1, "#e2c33b"],
    ];
    for (const [value, color] of defs) {
      const num = Number(value);
      if (Number.isFinite(num)) markers.push({ value: num, color });
    }
  }
  return markers;
}

export function resolvePreviewRendererKey(kind, { isText, isTu, isBool }) {
  if (isText && kind === "image") return "image";
  if (isText) return "text";
  if (kind === "button") return "button";
  if (kind === "led" || (kind === "auto" && isBool)) return "led";
  if (isTu) return "tu";
  if (kind === "bar") return "bar";
  if (kind === "gauge") return "gauge";
  if (kind === "setpoint") return "setpoint";
  if (kind === "numeric" || kind === "auto") return "numeric";
  if (kind === "trend") return "trend";
  return "fallback";
}

export function buildTrendPolylinePoints(samples, scaleMax, leftPad, innerW, topPad, innerH) {
  if (samples.length < 2) return "";
  return samples
    .map((sample, idx) => {
      const x = samples.length <= 1 ? leftPad : leftPad + (idx * innerW) / (samples.length - 1);
      const norm = scaleMax > 0 ? Math.max(0, Math.min(1, Number(sample) / scaleMax)) : 0;
      const y = topPad + innerH - norm * innerH;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
}

export function escapeHtml(s) {
  if (s === null || s === undefined) return "";
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function imageSrcFromBinding(rawLabel, binding = {}) {
  const raw = String(rawLabel || "").trim().replace(/\\/g, "/");
  if (!raw) return "";
  const allowedExt = /\.(png|jpe?g|gif|webp|svg)(\?|#|$)/i;
  if (/^[a-z]+:\/\//i.test(raw)) {
    if (!/^https?:\/\//i.test(raw)) return "";
    if (!allowedExt.test(raw.split("?")[0].split("#")[0])) return "";
    return raw;
  }
  const withoutPrefix = raw.replace(/^\/?ui_images\//i, "");
  if (withoutPrefix.includes("..") || /^[a-z]:\//i.test(withoutPrefix) || withoutPrefix.startsWith("/")) return "";
  if (!allowedExt.test(withoutPrefix)) return "";
  const version = encodeURIComponent([
    binding.x,
    binding.y,
    binding.w,
    binding.h,
    binding.fmt || "",
    binding.scale_max ?? "",
  ].join(":"));
  return `/ui_images/${withoutPrefix.split("/").map(encodeURIComponent).join("/")}?v=${version}`;
}

export function fmtStatValue(value) {
  if (!Number.isFinite(value)) return "-";
  const abs = Math.abs(value);
  if (abs >= 1000) return value.toFixed(0);
  if (abs >= 100) return value.toFixed(1);
  return value.toFixed(3);
}

export function fmtStatTime(unixSec) {
  if (!Number.isFinite(unixSec)) return "-";
  const dt = new Date(unixSec * 1000);
  const yyyy = dt.getFullYear();
  const mm = String(dt.getMonth() + 1).padStart(2, "0");
  const dd = String(dt.getDate()).padStart(2, "0");
  const hh = String(dt.getHours()).padStart(2, "0");
  const mi = String(dt.getMinutes()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}`;
}

export function chartTitle(subtext) {
  return {
    text: "ARX values",
    subtext,
    left: 10,
    top: 8,
    textStyle: { color: "#eef4fb", fontSize: 18, fontWeight: 700 },
    subtextStyle: { color: "#8fa6bf", fontSize: 12 },
  };
}

export function buildSeries(rows, showSymbols = false, smoothLines = false) {
  return (rows || []).map((seriesRow) => ({
    name: `Reg ${seriesRow.reg_id}`,
    type: "line",
    showSymbol: showSymbols,
    symbol: "circle",
    symbolSize: showSymbols ? 5 : 4,
    smooth: smoothLines,
    lineStyle: { width: 2 },
    emphasis: { focus: "series" },
    data: (seriesRow.points || []).map((point) => [point.ts_unix * 1000, point.val_num]),
  }));
}

export function collectVisibleIds(uiBindings) {
  return [...new Set(
    uiBindings
      .filter((x) => x.visible && !x.is_text && x.reg_id > 0)
      .map((x) => x.reg_id)
  )];
}

export function clampScale(nextScale) {
  return Math.max(0.25, Math.min(4, Number(nextScale) || 1));
}

/**
 * Minimal controller simulating showConfirmModal / closeConfirmModal
 * Promise resolution logic from preview_modals.js.
 * Added to test the fix: resolve(true) must happen BEFORE closeConfirmModal
 * (which would otherwise resolve(false) and leave the promise settled as false).
 */
export function createConfirmController() {
  let confirmResolve = null;

  function closeConfirmModal() {
    if (confirmResolve) {
      confirmResolve(false);
      confirmResolve = null;
    }
  }

  function showConfirmModal() {
    return new Promise((resolve) => {
      closeConfirmModal();
      confirmResolve = resolve;
    });
  }

  /** Simulates the Send button click — FIXED order */
  function clickSend() {
    // IMPORTANT: resolve(true) BEFORE closeConfirmModal
    const r = confirmResolve;
    confirmResolve = null;
    if (r) r(true);
  }

  /** Simulates the Cancel button click */
  function clickCancel() {
    closeConfirmModal();
  }

  function isPending() {
    return confirmResolve !== null;
  }

  return { showConfirmModal, closeConfirmModal, clickSend, clickCancel, isPending };
}

export function buildDetailFromRows(rows, valueForStatus, cleanHex, maxPreview = 12) {
  const parts = [];
  for (let i = 0; i < rows.length && i < maxPreview; i++) {
    parts.push(`${rows[i].reg_id}=${valueForStatus(rows[i])}`);
  }
  return {
    detail: `${rows.length} regs`,
    debug: parts.length ? `Values: ${parts.join(", ")}${rows.length > maxPreview ? ", ..." : ""}` : "",
  };
}
