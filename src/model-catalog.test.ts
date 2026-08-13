import { describe, expect, it } from "vitest";
import {
  applyCachedCatalogs,
  mergeCatalog,
  parseOpenRouterModels,
  type ModelCatalogs,
} from "./model-catalog";

describe("parseOpenRouterModels", () => {
  it("沒有 name 的拿 id 當顯示名，沒有 id 的整筆丟掉", () => {
    expect(
      parseOpenRouterModels({
        data: [
          { id: "openai/gpt-5.5", name: "OpenAI: GPT-5.5" },
          { id: "anthropic/claude-opus-5" },
          { name: "沒有 id" },
          { id: "" },
        ],
      }),
    ).toEqual([
      { id: "openai/gpt-5.5", label: "OpenAI: GPT-5.5" },
      { id: "anthropic/claude-opus-5", label: "anthropic/claude-opus-5" },
    ]);
  });

  it("回應不成形狀就回空陣列，不炸掉預熱", () => {
    expect(parseOpenRouterModels(null)).toEqual([]);
    expect(parseOpenRouterModels({})).toEqual([]);
    expect(parseOpenRouterModels({ data: "not an array" })).toEqual([]);
  });
});

describe("mergeCatalog", () => {
  const store: ModelCatalogs = { grok: [{ id: "grok-4.6", label: "grok-4.6 (default)" }] };

  it("抓到新清單就換上", () => {
    expect(mergeCatalog(store, "grok", [{ id: "grok-5", label: "grok-5" }])).toEqual({
      grok: [{ id: "grok-5", label: "grok-5" }],
    });
  });

  // 沒登入／斷網／子行程逾時都會回空清單，這時清空下拉等於把玩家能選的東西拿走
  it("抓到空清單就留著上次那份", () => {
    expect(mergeCatalog(store, "grok", [])).toEqual(store);
  });

  it("別家的結果不影響既有的", () => {
    const merged = mergeCatalog(store, "agy", [{ id: "gemini-3.6-flash-high", label: "Gemini" }]);
    expect(merged.grok).toEqual(store.grok);
    expect(merged.agy).toEqual([{ id: "gemini-3.6-flash-high", label: "Gemini" }]);
  });
});

describe("applyCachedCatalogs", () => {
  it("快取檔補上還沒抓到的那幾家", () => {
    const cached: ModelCatalogs = { agy: [{ id: "gemini", label: "Gemini" }] };
    expect(applyCachedCatalogs({}, cached)).toEqual(cached);
  });

  // 快取檔讀得比某支抓取還慢時，舊的不可以蓋掉已經抓回來的新結果
  it("已經抓回來的新結果優先於快取檔", () => {
    const fresh: ModelCatalogs = { grok: [{ id: "grok-5", label: "grok-5" }] };
    const cached: ModelCatalogs = { grok: [{ id: "grok-4.6", label: "grok-4.6" }] };
    expect(applyCachedCatalogs(fresh, cached).grok).toEqual(fresh.grok);
  });
});
