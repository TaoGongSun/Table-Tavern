// @vitest-environment happy-dom
// 狀態樹的分支指認下拉只掛在名字鍵的物件分支：清單容器、清單項（含 `0` 那一支本身）與其底下都不掛。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setLang, t } from "../../i18n";
import { loadGlyphEngine, presentItems } from "./present-glyphs";
import { StateBar } from "./StateBar";
import { displayWithFormat, parseWithFormat, prefixFor, stripPrefix, withPrefix } from "./state-format";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  // Node 新版的全域 localStorage 在沒給 --localstorage-file 時是 undefined，蓋掉 happy-dom 的
  const store = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => void store.set(key, String(value)),
  });
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  vi.unstubAllGlobals();
});

const noop = () => {};

function renderTree(tree: Record<string, unknown>, displayFormats?: Record<string, string>) {
  act(() =>
    root.render(
      <StateBar
        fields={{}}
        tree={tree as never}
        jumps={{}}
        displayFormats={displayFormats}
        bindings={[]}
        editing={null}
        onBeginEdit={noop}
        onChangeEditValue={noop}
        onSave={noop}
        onCancelEdit={noop}
        onMarkCounter={noop}
        onBind={noop}
        player={null}
        castCount={1}
        cast={[{ id: "c1", name: "林远" } as never]}
      />,
    ),
  );
}

function branchesWithSelect(): string[] {
  return [...host.querySelectorAll("details.state-tree-branch > summary")]
    .filter((summary) => summary.querySelector("select"))
    .map((summary) => summary.firstChild?.textContent ?? "");
}

describe("StateBar 分支指認下拉", () => {
  it("清單容器、清單項（含 0 那一支）與其底下都不掛；名字鍵與年份鍵分支照掛", () => {
    renderTree({
      状态栏: {
        人物列表: {
          "0": { 人物: { 名字: "林远" } },
          "1": { 人物: { 名字: "贺铮" } },
        },
        驿站物资: { 粮草: "四百石" },
        年度: { "2025": { 收成: "平" }, "2026": { 收成: "丰" } },
      },
    });
    expect(branchesWithSelect().sort()).toEqual(["2025", "2026", "年度", "状态栏", "驿站物资"].sort());
  });

  it("接管桌具名集合：項目照 _序 排、不顯示 _序、集合與項目都不掛下拉", () => {
    renderTree({
      在場人物: {
        周掌柜: { _序: "2", 身份: "军需官" },
        林远: { _序: "0", 身份: "新任驿丞" },
        贺铮: { _序: "1", 身份: "老驿长" },
      },
      驿站: { 粮草: "四百石" },
    });
    const summaries = [...host.querySelectorAll("details.state-tree-branch > summary")].map(
      (summary) => summary.firstChild?.textContent ?? "",
    );
    expect(summaries).toEqual(["在場人物", "林远", "贺铮", "周掌柜", "驿站"]);
    expect(host.textContent).not.toContain("_序");
    expect(branchesWithSelect()).toEqual(["驿站"]);
  });

  it("空分支顯示空值字樣，根層空樹不印", () => {
    renderTree({ 名單: {} });
    expect(host.querySelector(".state-tree-empty")?.textContent).toBe(t("stateEmptyValue"));
    renderTree({});
    expect(host.querySelector(".state-tree-empty")).toBeNull();
  });
});

describe("StateBar 數字欄顯示模板", () => {
  it("樹上存數字、畫面照原寫法顯示；點開編輯仍是數字；值不是純數字就原樣", () => {
    expect(displayWithFormat("7", "第 {} 天")).toBe("第 7 天");
    expect(displayWithFormat("第 7 天", "第 {} 天")).toBe("第 7 天");
    expect(displayWithFormat("7", undefined)).toBe("7");
    renderTree({ 驻守天数: "7" }, { 驻守天数: "第 {} 天" });
    const values = [...host.querySelectorAll(".state-bar-value")].map((node) => node.textContent);
    expect(values).toContain("第 7 天");
    expect(values).not.toContain("7");
  });

  it("輸入照模板解析回數字（空白有無都算），不符合就原樣", () => {
    expect(parseWithFormat("第 8 天", "第 {} 天")).toBe("8");
    expect(parseWithFormat("第8天", "第 {} 天")).toBe("8");
    expect(parseWithFormat("8", "第 {} 天")).toBe("8");
    expect(parseWithFormat("第八天", "第 {} 天")).toBe("第八天");
    expect(parseWithFormat("第 天", "第 {} 天")).toBe("第 天");
    expect(parseWithFormat("第 8 夜", "第 {} 天")).toBe("第 8 夜");
    expect(parseWithFormat("第 8 天", undefined)).toBe("第 8 天");
  });
});

function renderPresent(present: string, onBeginEdit: (path: string[], tree: boolean, value: string) => void = noop) {
  act(() =>
    root.render(
      <StateBar
        fields={{ time: "夜", place: "前院", present }}
        tree={{}}
        jumps={{}}
        bindings={[]}
        editing={null}
        onBeginEdit={onBeginEdit}
        onChangeEditValue={noop}
        onSave={noop}
        onCancelEdit={noop}
        onMarkCounter={noop}
        onBind={noop}
        player={{ id: "p", name: "林远" } as never}
        castCount={1}
        cast={[]}
        knownNames={["林远", "贺铮", "周掌柜", "阿古拉"]}
      />,
    ),
  );
}

async function settle(target: "tw" | "cn") {
  await act(async () => {
    await loadGlyphEngine(target);
    await Promise.resolve();
  });
}

const summaryPresent = () => [...host.querySelectorAll("summary .state-bar-seg")][2] as HTMLElement;
const fieldButton = (index: number) => host.querySelectorAll<HTMLButtonElement>(".state-bar-value")[index];

describe("StateBar present 顯示字形", () => {
  afterEach(() => setLang("zh-TW"));

  it("zh-TW：收合摘要、title、展開欄位都轉繁；玩家巨集先代換；點開編輯是原字", async () => {
    setLang("zh-TW");
    const begin = vi.fn();
    renderPresent("{{user}}、贺铮、周掌櫃", begin);
    await settle("tw");
    renderPresent("{{user}}、贺铮、周掌櫃", begin);
    expect(summaryPresent().textContent).toContain("林遠、賀錚、周掌櫃");
    expect(summaryPresent().title).toContain("林遠、賀錚、周掌櫃");
    expect(fieldButton(2).textContent).toBe("林遠、賀錚、周掌櫃");
    act(() => fieldButton(2).click());
    expect(begin).toHaveBeenCalledWith(["present"], false, "{{user}}、贺铮、周掌櫃");
    expect(fieldButton(0).textContent).toBe("夜");
  });

  it("zh-CN：繁體轉簡，沈鬱不變成沉郁", async () => {
    setLang("zh-CN");
    renderPresent("賀錚、沈鬱");
    await settle("cn");
    expect(fieldButton(2).textContent).toBe("贺铮、沈郁");
  });

  it("非中文介面不轉；切換語系即時改顯示", async () => {
    setLang("en");
    renderPresent("贺铮、周掌柜");
    expect(fieldButton(2).textContent).toBe("贺铮、周掌柜");
    setLang("zh-TW");
    renderPresent("贺铮、周掌柜");
    await settle("tw");
    expect(fieldButton(2).textContent).toBe("賀錚、周掌櫃");
    setLang("ja");
    renderPresent("贺铮、周掌柜");
    expect(fieldButton(2).textContent).toBe("贺铮、周掌柜");
  });

  it("載入完成前值已更新（或換桌）：顯示的是最新值的轉換", async () => {
    setLang("zh-TW");
    renderPresent("贺铮");
    renderPresent("阿古拉、周掌柜");
    await settle("tw");
    expect(fieldButton(2).textContent).toBe("阿古拉、周掌櫃");
  });
});

describe("StateBar 鏡像欄", () => {
  it("跟著固定鍵走的卡欄位照樣顯示但不給點開改，其他葉子照舊可改", () => {
    act(() =>
      root.render(
        <StateBar
          fields={{ place: "北坡" }}
          tree={{ 当前环境: "北坡", 剧情阶段: "初始" } as never}
          jumps={{}}
          mirrors={{ 当前环境: "place" }}
          bindings={[]}
          editing={null}
          onBeginEdit={noop}
          onChangeEditValue={noop}
          onSave={noop}
          onCancelEdit={noop}
          onMarkCounter={noop}
          onBind={noop}
          player={null}
          castCount={1}
          cast={[]}
        />,
      ),
    );
    const rows = [...host.querySelectorAll(".state-tree-children .state-bar-field, .state-bar-field")];
    const row = (label: string) =>
      rows.find((el) => el.querySelector(".state-bar-label")?.textContent === label);
    expect(row("当前环境")?.querySelector("button")).toBeNull();
    expect(row("当前环境")?.textContent).toContain("北坡");
    expect(row("剧情阶段")?.querySelector("button.state-bar-value")).not.toBeNull();
  });
});

describe("StateBar 顯示前綴", () => {
  const prefixes = {
    "状态栏.人物列表.*": "👤 ",
    "状态栏.人物列表.*.身份": "🪪 ",
    "状态栏.人物列表.林远.身份": "🧭 ",
    "*.*.身份": "❓ ",
    "状态栏.天气": "🌨️ ",
    驻守天数: "📅 ",
  };

  it("路徑比對：精確優先，再找段數相同、萬用段最少的", () => {
    expect(prefixFor(prefixes, ["状态栏", "人物列表", "林远", "身份"])).toBe("🧭 ");
    expect(prefixFor(prefixes, ["状态栏", "人物列表", "贺铮", "身份"])).toBe("🪪 ");
    expect(prefixFor(prefixes, ["别处", "贺铮", "身份"])).toBe("❓ ");
    expect(prefixFor(prefixes, ["状态栏", "人物列表", "贺铮", "心情"])).toBeUndefined();
  });

  it("顯示接回前綴、值已帶前綴不重複接；輸入帶前綴就剝掉", () => {
    expect(withPrefix("新任驿丞", "🪪 ")).toBe("🪪 新任驿丞");
    expect(withPrefix("🪪 新任驿丞", "🪪 ")).toBe("🪪 新任驿丞");
    expect(withPrefix("", "🪪 ")).toBe("");
    expect(stripPrefix("🪪 斥候队长", "🪪 ")).toBe("斥候队长");
    expect(stripPrefix("🪪斥候队长", "🪪 ")).toBe("斥候队长");
    expect(stripPrefix("斥候队长", "🪪 ")).toBe("斥候队长");
  });

  it("狀態列：葉子接前綴、集合項目標題接名字欄前綴；先套數字模板再接前綴", () => {
    act(() =>
      root.render(
        <StateBar
          fields={{}}
          tree={
            {
              状态栏: {
                天气: "北风小雪",
                人物列表: { 林远: { _序: "0", 身份: "新任驿丞" }, 贺铮: { _序: "1", 身份: "🪪 老驿长" } },
              },
              驻守天数: "7",
            } as never
          }
          jumps={{}}
          displayFormats={{ 驻守天数: "第 {} 天" }}
          displayPrefixes={prefixes}
          bindings={[]}
          editing={null}
          onBeginEdit={noop}
          onChangeEditValue={noop}
          onSave={noop}
          onCancelEdit={noop}
          onMarkCounter={noop}
          onBind={noop}
          player={null}
          castCount={0}
          cast={[]}
        />,
      ),
    );
    const values = [...host.querySelectorAll(".state-bar-value")].map((node) => node.textContent);
    expect(values).toContain("🌨️ 北风小雪");
    expect(values).toContain("🧭 新任驿丞");
    expect(values).toContain("🪪 老驿长");
    expect(values).toContain("📅 第 7 天");
    const summaries = [...host.querySelectorAll("details.state-tree-branch > summary")].map(
      (summary) => summary.firstChild?.textContent ?? "",
    );
    expect(summaries).toContain("👤 林远");
    expect(summaries).toContain("人物列表");
  });
});

describe("StateBar 在場名單集合", () => {
  const people = {
    林远: { _序: "0", 心情: "警觉" },
    贺铮: { _序: "1", 心情: "冷淡" },
    周掌柜: { _序: "2", 心情: "殷勤" },
  };

  function titles(present: string, scoped: string[]) {
    act(() =>
      root.render(
        <StateBar
          fields={{ present }}
          tree={{ 状态栏: { 人物列表: people } } as never}
          jumps={{}}
          presentScoped={scoped}
          bindings={[]}
          editing={null}
          onBeginEdit={noop}
          onChangeEditValue={noop}
          onSave={noop}
          onCancelEdit={noop}
          onMarkCounter={noop}
          onBind={noop}
          player={{ name: "林远" } as never}
          castCount={0}
          cast={[]}
        />,
      ),
    );
    return [...host.querySelectorAll("details.state-tree-branch > summary")]
      .map((summary) => summary.firstChild?.textContent ?? "")
      .filter((title) => !["状态栏", "人物列表"].includes(title));
  }

  it("只列 present 裡的人；離場的回場再列；{{user}} 照玩家名比", () => {
    expect(titles("{{user}}、周掌柜", ["状态栏.人物列表"])).toEqual(["林远", "周掌柜"]);
    expect(titles("林远、贺铮、周掌柜", ["状态栏.人物列表"])).toEqual(["林远", "贺铮", "周掌柜"]);
  });

  it("沒標在場名單、或還沒有在場資料就全列", () => {
    expect(titles("林远", [])).toEqual(["林远", "贺铮", "周掌柜"]);
    expect(titles("", ["状态栏.人物列表"])).toEqual(["林远", "贺铮", "周掌柜"]);
  });

  it("已有明確名單卻一個都對不上：集合是空的，不把離場的人列回來", () => {
    expect(titles("某路人", ["状态栏.人物列表"])).toEqual([]);
    expect(host.textContent).toContain(t("stateEmptyValue"));
  });

  it("名字字形不同時用折疊比對", () => {
    const fold = (text: string) => text.replace("遠", "远").replace("賀", "贺");
    const entries: [string, number][] = [
      ["林远", 0],
      ["贺铮", 1],
    ];
    expect(presentItems(entries, "林遠", fold)).toEqual([["林远", 0]]);
    expect(presentItems(entries, "林遠", null)).toEqual([]);
    expect(presentItems(entries, "", null)).toEqual(entries);
  });
});
