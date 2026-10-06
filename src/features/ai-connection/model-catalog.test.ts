import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "../../i18n";
import {
  applyCachedCatalogs,
  CLI_DEFAULT_MODEL,
  CATALOG_SOURCES,
  mergeCatalog,
  parseOpenRouterImageModels,
  parseOpenRouterModels,
  usageModelLabel,
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

  it("帶出換幕容量用的 context：模型與 top_provider 取小、輸出上限；缺或不合理就不帶", () => {
    expect(
      parseOpenRouterModels({
        data: [
          {
            id: "a/x",
            name: "X",
            context_length: 200000,
            top_provider: { context_length: 131072, max_completion_tokens: 8192 },
          },
          { id: "a/y", context_length: 64000, top_provider: { context_length: null } },
          { id: "a/z", context_length: "big" },
        ],
      }),
    ).toEqual([
      { id: "a/x", label: "X", context_tokens: 131072, max_output_tokens: 8192 },
      { id: "a/y", label: "a/y", context_tokens: 64000 },
      { id: "a/z", label: "a/z" },
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

describe("usageModelLabel", () => {
  afterEach(() => setLang("zh-TW"));

  it("後端存的 CLI 預設字樣換成目前語系，其餘模型 id 原樣", () => {
    setLang("ru");
    expect(usageModelLabel(CLI_DEFAULT_MODEL)).toBe("CLI по умолч.");
    expect(usageModelLabel("opus")).toBe("opus");
    setLang("zh-TW");
    expect(usageModelLabel("(CLI 預設)")).toBe("CLI 預設");
    // 只認整串相等，含這幾個字的其他 id 不動
    expect(usageModelLabel("x (CLI 預設)")).toBe("x (CLI 預設)");
  });
});

describe("parseOpenRouterImageModels", () => {
  it("生圖清單排除 openrouter/auto 路由器，其餘照官方名稱原樣", () => {
    expect(
      parseOpenRouterImageModels({
        data: [
          { id: "openrouter/auto-beta", name: "Auto Router" },
          { id: "google/gemini-3.1-flash-image", name: "Google: Gemini Flash Image" },
          { id: "inclusionai/ming-image-0.1-design", name: "Ming Image (free)" },
        ],
      }),
    ).toEqual([
      { id: "google/gemini-3.1-flash-image", label: "Google: Gemini Flash Image" },
      { id: "inclusionai/ming-image-0.1-design", label: "Ming Image (free)" },
    ]);
  });

  it("生圖清單跟其他清單一起預熱", () => {
    expect(CATALOG_SOURCES).toContain("api-image");
  });
});
