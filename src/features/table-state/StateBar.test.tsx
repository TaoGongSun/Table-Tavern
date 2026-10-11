// @vitest-environment happy-dom
// 狀態樹的分支指認下拉只掛在名字鍵的物件分支：清單容器、清單項（含 `0` 那一支本身）與其底下都不掛。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import { StateBar } from "./StateBar";

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

function renderTree(tree: Record<string, unknown>) {
  act(() =>
    root.render(
      <StateBar
        fields={{}}
        tree={tree as never}
        jumps={{}}
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

  it("空分支顯示空值字樣，根層空樹不印", () => {
    renderTree({ 名單: {} });
    expect(host.querySelector(".state-tree-empty")?.textContent).toBe(t("stateEmptyValue"));
    renderTree({});
    expect(host.querySelector(".state-tree-empty")).toBeNull();
  });
});
