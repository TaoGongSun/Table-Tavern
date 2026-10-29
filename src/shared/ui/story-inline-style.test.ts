import { describe, expect, it } from "vitest";
import { sanitizeInlineStyle } from "./story-inline-style";

describe("sanitizeInlineStyle", () => {
  it("keeps the allowlisted properties with literal values", () => {
    expect(sanitizeInlineStyle("color:#f00; background-color:#11223344; font-weight:bold; font-style:italic")).toBe(
      "color: #f00; background-color: #11223344; font-weight: bold; font-style: italic",
    );
    expect(sanitizeInlineStyle("COLOR : Red ;text-decoration:underline wavy #abcdef")).toBe(
      "color: Red; text-decoration: underline wavy #abcdef",
    );
    expect(sanitizeInlineStyle("background-color:rgba(0, 0, 0, .5);color:hsl(120deg,50%,50%)")).toBe(
      "background-color: rgba(0, 0, 0, .5); color: hsl(120deg,50%,50%)",
    );
    expect(sanitizeInlineStyle("color:red !important; font-weight:700")).toBe("color: red; font-weight: 700");
  });

  it("bounds font-size through a generated clamp, keywords included", () => {
    expect(sanitizeInlineStyle("font-size:1.5em")).toBe("font-size: clamp(8px, 1.5em, 32px)");
    expect(sanitizeInlineStyle("font-size:120%;font-size:20px")).toBe(
      "font-size: clamp(8px, 120%, 32px); font-size: clamp(8px, 20px, 32px)",
    );
    expect(sanitizeInlineStyle("font-size:larger")).toBe("font-size: clamp(8px, 1.2em, 32px)");
    expect(sanitizeInlineStyle("font-size:x-large")).toBe("font-size: x-large");
    expect(sanitizeInlineStyle("font-size:10vw")).toBe("");
  });

  it("drops only the declarations outside the allowlist", () => {
    expect(sanitizeInlineStyle("position:fixed;top:0;color:blue;width:100vw;constructor:x")).toBe("color: blue");
    expect(sanitizeInlineStyle("background:#000;display:none")).toBe("");
  });

  it("drops the whole attribute on functions, escapes, quotes and comments", () => {
    for (const raw of [
      "color:red;background-image:url(https://e.com/x)",
      "background-color:image-set(x 1x)",
      "color:expression(alert(1))",
      "color:rgb(url(x))",
      "color:xrgb(1,2,3)",
      "color:\\75rl(x)",
      'color:"red"',
      "color:red/**/;font-weight:bold",
      "color:red;}body{color:red",
      "font-size:calc(100vw)",
    ]) {
      expect(sanitizeInlineStyle(raw), raw).toBe("");
    }
  });
});
