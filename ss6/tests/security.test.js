import { describe, it, expect } from "vitest";
import { escapeHtml, imageSrcFromBinding } from "./pure.js";

describe("escapeHtml", () => {
  it("returns empty for null/undefined", () => {
    expect(escapeHtml(null)).toBe("");
    expect(escapeHtml(undefined)).toBe("");
  });

  it("escapes ampersands", () => {
    expect(escapeHtml("a&b")).toBe("a&amp;b");
    expect(escapeHtml("&&&")).toBe("&amp;&amp;&amp;");
  });

  it("escapes angle brackets", () => {
    expect(escapeHtml("<script>")).toBe("&lt;script&gt;");
    expect(escapeHtml("<img onerror=alert(1)>")).toBe("&lt;img onerror=alert(1)&gt;");
  });

  it("escapes quotes", () => {
    expect(escapeHtml('"hello"')).toBe("&quot;hello&quot;");
    expect(escapeHtml("'hello'")).toBe("&#39;hello&#39;");
  });

  it("escapes all dangerous characters together", () => {
    const input = `<img src=x onerror="alert('XSS')">`;
    const result = escapeHtml(input);
    expect(result).not.toContain("<");
    expect(result).not.toContain(">");
    expect(result).not.toContain('"');
    expect(result).not.toContain("'");
    expect(result).toContain("&lt;");
    expect(result).toContain("&gt;");
    expect(result).toContain("&quot;");
    expect(result).toContain("&#39;");
  });

  it("preserves safe text", () => {
    expect(escapeHtml("Temperature 100°C")).toBe("Temperature 100°C");
    expect(escapeHtml("abc 123")).toBe("abc 123");
  });

  it("handles numeric input", () => {
    expect(escapeHtml(42)).toBe("42");
    expect(escapeHtml(0)).toBe("0");
    expect(escapeHtml(-1.5)).toBe("-1.5");
  });
});

describe("imageSrcFromBinding", () => {
  it("returns empty for empty input", () => {
    expect(imageSrcFromBinding("")).toBe("");
    expect(imageSrcFromBinding(null)).toBe("");
    expect(imageSrcFromBinding("  ")).toBe("");
  });

  it("returns empty for non-http protocols", () => {
    expect(imageSrcFromBinding("javascript:alert(1)")).toBe("");
    expect(imageSrcFromBinding("file:///etc/passwd")).toBe("");
    expect(imageSrcFromBinding("ftp://evil.com/image.png")).toBe("");
  });

  it("blocks remote URLs with disallowed extensions", () => {
    expect(imageSrcFromBinding("https://evil.com/image.php?x=1")).toBe("");
    expect(imageSrcFromBinding("http://evil.com/script.js")).toBe("");
    expect(imageSrcFromBinding("https://evil.com/image")).toBe("");
  });

  it("allows http/https URLs with valid image extensions", () => {
    expect(imageSrcFromBinding("https://example.com/image.png")).toMatch(/^https:\/\//);
    expect(imageSrcFromBinding("https://example.com/photo.jpg")).toMatch(/^https:\/\//);
    expect(imageSrcFromBinding("https://example.com/img.jpeg")).toMatch(/^https:\/\//);
    expect(imageSrcFromBinding("https://example.com/img.gif")).toMatch(/^https:\/\//);
    expect(imageSrcFromBinding("https://example.com/img.webp")).toMatch(/^https:\/\//);
    expect(imageSrcFromBinding("https://example.com/img.svg")).toMatch(/^https:\/\//);
  });

  it("allows http URLs with valid extension", () => {
    expect(imageSrcFromBinding("http://example.com/scheme.png")).toMatch(/^http:\/\//);
  });

  it("blocks local paths with .. traversal", () => {
    expect(imageSrcFromBinding("ui_images/../../../etc/passwd")).toBe("");
    expect(imageSrcFromBinding("/ui_images/../../../secret")).toBe("");
  });

  it("blocks absolute Windows paths", () => {
    expect(imageSrcFromBinding("C:\\windows\\system32\\evil.png")).toBe("");
    expect(imageSrcFromBinding("D:\\secret.png")).toBe("");
  });

  it("blocks root-relative paths", () => {
    expect(imageSrcFromBinding("/etc/passwd")).toBe("");
  });

  it("blocks local paths with disallowed extensions", () => {
    expect(imageSrcFromBinding("ui_images/script.js")).toBe("");
    expect(imageSrcFromBinding("ui_images/page.html")).toBe("");
    expect(imageSrcFromBinding("ui_images/data.php")).toBe("");
  });

  it("builds correct path for valid ui_images local path", () => {
    const binding = { x: 10, y: 20, w: 100, h: 50, fmt: "png", scale_max: 100 };
    const result = imageSrcFromBinding("ui_images/scheme.png", binding);
    expect(result).toContain("/ui_images/scheme.png?v=");
    expect(result).toContain(encodeURIComponent("10:20:100:50:png:100"));
  });

  it("strips leading /ui_images/ prefix", () => {
    const binding = { x: 0, y: 0, w: 10, h: 10 };
    const result = imageSrcFromBinding("/ui_images/photo.png", binding);
    expect(result).toContain("/ui_images/photo.png?v=");
  });

  it("encodes path components", () => {
    const binding = { x: 0, y: 0, w: 10, h: 10, fmt: "", scale_max: 50 };
    const result = imageSrcFromBinding("ui_images/sub dir/img.png", binding);
    expect(result).toContain("/ui_images/sub%20dir/img.png?v=");
  });

  it("handles URLs with query strings", () => {
    expect(imageSrcFromBinding("https://example.com/img.png?w=200")).toMatch(/^https:\/\//);
    expect(imageSrcFromBinding("https://example.com/img.png#fragment")).toMatch(/^https:\/\//);
  });
});
