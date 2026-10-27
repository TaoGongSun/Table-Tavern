import { describe, expect, it } from "vitest";
import { KEEP, leftoverCapabilities, stripGlobals } from "./strip";

describe("隔離 Worker 的全域剝除", () => {
  it("沿原型鏈刪掉白名單外的一切，留下內建；刪不掉的回報", () => {
    const base = Object.create(Object.prototype, { fetch: { value: () => 1, configurable: true }, TEMPORARY: { value: 0, configurable: false } });
    const middle = Object.create(base, { indexedDB: { get: () => ({}), configurable: true } });
    const scope = Object.create(middle, { JSON: { value: JSON, configurable: true }, Worker: { value: class {}, configurable: true } });
    expect(stripGlobals(scope)).toEqual(["TEMPORARY"]);
    expect(Object.getOwnPropertyNames(scope)).toEqual(["JSON"]);
    expect(leftoverCapabilities(scope)).toEqual([]);
    expect(KEEP.has("TextEncoder")).toBe(true);
  });
  it("失敗即停：還看得到任何危險能力就列出來", () => {
    expect(leftoverCapabilities({ fetch: () => 1, postMessage: () => 1, ok: 1 })).toEqual(["fetch", "postMessage"]);
    const throwing = Object.defineProperty({}, "indexedDB", {
      get() {
        throw new Error("x");
      },
    });
    expect(leftoverCapabilities(throwing)).toEqual(["indexedDB"]);
  });
});
