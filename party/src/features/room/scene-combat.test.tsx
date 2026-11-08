// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeAll, expect, it } from "vitest";
import type { ScenePanel } from "../../../server/core/types";
import { setLangOnly } from "../../i18n";
import { SceneStatusLine, SceneStatusSheet } from "./scene-status";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  setLangOnly("zh-TW");
});

function render(panel: ScenePanel) {
  const el = document.body.appendChild(document.createElement("div"));
  const root = createRoot(el);
  const parts = [{ state: { time: "夜", place: "洞窟" }, panel }];
  act(() =>
    root.render(
      <div className="app">
        <SceneStatusLine parts={parts} open onOpen={() => {}} />
        <SceneStatusSheet parts={parts} onClose={() => {}} />
      </div>,
    ),
  );
  return el;
}

it("手機收合列顯示輪到誰或戰鬥中，抽屜唯讀列先攻、敵人與 HP", () => {
  const el = render({
    characters: { 沃恩: { status: "喘氣", hp: "18/24", conditions: ["中毒"] } },
    combat: { order: ["沃恩", "哥布林A"], turn: "哥布林A", foes: { 哥布林A: { hp: "3/7" } } },
  });
  const line = el.querySelector('[data-testid="scene-status"]')!;
  expect(line.querySelector(".scene-status-item")!.textContent).toBe("輪到 哥布林A");
  expect(line.textContent).not.toContain("⚔");
  const sheet = el.querySelector('[data-testid="status-sheet"]')!;
  expect(sheet.querySelector("li.is-turn")!.textContent).toBe("哥布林A");
  expect(sheet.textContent).toContain("HP 18/24");
  expect(sheet.textContent).toContain("中毒");
  expect(sheet.querySelector(".scene-panel-foes li")!.textContent).toBe("哥布林A3/7");
  expect(sheet.querySelectorAll("input, textarea, li button")).toHaveLength(0);
  const waiting = render({ combat: { order: ["沃恩"], turn: "", foes: {} } });
  expect(waiting.querySelector('[data-testid="scene-status"]')!.textContent).toContain("戰鬥中");
});
