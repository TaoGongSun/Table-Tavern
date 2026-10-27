// @vitest-environment happy-dom
// 狀態樹的分支指認下拉只掛在名字鍵的物件分支：清單容器、清單項（含 `0` 那一支本身）與其底下都不掛。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setLang, t } from "../../i18n";
import { loadGlyphEngine } from "./present-glyphs";
import { StateBar } from "./StateBar";
import { displayWithFormat, parseWithFormat } from "./state-format";

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
