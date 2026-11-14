// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { setLang } from "../../i18n";
import { type StateNode, type WorldState } from "../../shared/contracts/backend-contracts";
import { StateBar } from "./StateBar";
import { useTableStateController, type TableStateController } from "./useTableStateController";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
let host: HTMLDivElement, root: Root, ctl: TableStateController, state: WorldState;

function Probe() {
  ctl = useTableStateController({ worldId: "table", onError: () => {} });
  return (
    <StateBar
      fields={ctl.fields}
      tree={ctl.tree}
      jumps={ctl.jumps}
      defaultPanel={ctl.defaultPanel}
      bindings={[]}
      editing={ctl.editing}
      onBeginEdit={ctl.beginEdit}
      onChangeEditValue={ctl.changeEditValue}
      onSave={(path, tree, value) => void ctl.save(path, tree, value)}
      onCancelEdit={ctl.cancelEdit}
      onMarkCounter={() => {}}
      onBind={() => {}}
      onSetPanelList={(path, items) => void ctl.setPanelList(path, items)}
      onSetLeaf={(path, value) => void ctl.setLeaf(path, value)}
      changes={ctl.changes}
      refused={ctl.refused}
      combatEnded={ctl.combatEnded}
      player={null}
      castCount={0}
      cast={[]}
    />
  );
}

function world(combat: Record<string, StateNode>, extra: Partial<WorldState["state"]> = {}): WorldState {
  return {
    id: "table",
    name: "戰鬥桌",
    player_card_id: null,
    model_bindings: {},
    current_scene: 0,
    catchup_summaries: {},
    scene_titles: {},
    scene_labels: {},
    table_kind: "chat",
    state: {
      table: { time: "夜", place: "洞窟", present: "沃恩、雷克" },
      tree: {
        characters: { 沃恩: { status: "", items: {}, hp: "18/24", conditions: { "0": "中毒" } } },
        tracking: { goals: {}, promises: {}, clues: {} },
        combat,
      },
      ...extra,
    },
    mechanism: { default_panel: true, combat_panel: true },
  };
}

async function load(next: WorldState) {
  state = next;
  await act(async () => ctl.refresh());
}

beforeEach(async () => {
  setLang("zh-TW");
  vi.stubGlobal("localStorage", { getItem: () => "true", setItem: vi.fn() });
  vi.mocked(invoke).mockReset();
  state = world({ order: {}, turn: "", foes: {} });
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "read_state" ? state : command === "branch_bindings" ? [] : undefined,
  );
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root.render(<Probe />));
  await act(async () => ctl.refresh());
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
  vi.unstubAllGlobals();
});

const summary = () => host.querySelector("summary")!.textContent ?? "";
const button = (label: string) =>
  [...host.querySelectorAll("button")].find((b) => b.textContent === label || b.getAttribute("aria-label") === label)!;

it("沒在戰鬥時沒有戰鬥區；角色格多 HP 與狀態效果", () => {
  expect(host.querySelector(".state-combat")).toBeNull();
  expect(summary()).not.toContain("戰鬥中");
  const cell = host.querySelector('.state-panel-cell[aria-label="沃恩"]')!.textContent;
  expect(cell).toContain("HP18/24");
  expect(cell).toContain("狀態效果");
  expect(cell).toContain("中毒");
});

it("先攻有了沒輪到誰：收合列戰鬥中、列下提示；點名字設輪到誰、↑↓ 整串重寫", async () => {
  await load(world({ order: { "0": "沃恩", "1": "哥布林A" }, turn: "", foes: { 哥布林A: { hp: "7/7", conditions: {} } } }));
  expect(summary()).toContain("戰鬥中");
  expect(summary()).not.toMatch(/輪到\s*$/);
  expect(summary()).not.toContain("⚔");
  expect(host.querySelector(".state-combat-hint")?.textContent).toBe("點名字設定輪到誰");
  expect(host.querySelector('.state-panel-cell[aria-label="哥布林A"]')?.textContent).toContain("HP7/7");
  await act(async () => button("哥布林A").click());
  expect(invoke).toHaveBeenCalledWith("set_state_path", { worldId: "table", path: ["combat", "turn"], value: "哥布林A" });
  await act(async () => button("下移 沃恩").click());
  expect(invoke).toHaveBeenCalledWith("set_panel_list", {
    worldId: "table",
    path: ["combat", "order"],
    items: ["哥布林A", "沃恩"],
  });
  expect(button("上移 沃恩").disabled).toBe(true);
  await load(world({ order: { "0": "沃恩", "1": "哥布林A" }, turn: "哥布林A", foes: {} }));
  expect(summary()).toContain("輪到 哥布林A");
  expect(host.querySelector("li.is-turn")?.textContent).toContain("哥布林A");
});

it("HP 淨值標記；依骰面改套是灰色說明、真拒收帶警告且點開就進手改", async () => {
  await load(
    world(
      { order: { "0": "沃恩" }, turn: "沃恩", foes: {} },
      {
        changes: { "characters.沃恩.hp": "-12" },
        refused: {
          "characters.沃恩.hp": { wrote: "9.5/24" },
          "characters.沃恩.conditions": { wrote: "中毒" },
          "combat.foes.巨魔": { wrote: '{"hp":"40"}', reason: "duplicate" },
        },
      },
    ),
  );
  expect(host.querySelector(".state-bar-delta")?.textContent).toBe("-12");
  expect(host.querySelector(".state-combat")?.textContent).toContain("⚠ 巨魔 沒加上：同名敵人已存在");
  const badge = [...host.querySelectorAll<HTMLButtonElement>("button.state-refused")].find((b) =>
    b.textContent?.includes("9.5/24"),
  )!;
  expect(badge.textContent).toBe("⚠ GM 寫 9.5/24，未套用");
  await act(async () => badge.click());
  expect(host.querySelector<HTMLInputElement>(".state-bar-input")?.value).toBe("18/24");
  act(() => ctl.cancelEdit());
  await load(world({ order: {}, turn: "", foes: {} }, { refused: { "characters.沃恩.hp": { wrote: "-5", applied: "-7" } } }));
  const applied = host.querySelector(".state-dice-applied")!;
  expect(applied.textContent).toBe("依骰面 -7（GM 寫 -5）");
  expect(applied.textContent).not.toContain("⚠");
});

it("沒有先攻時拒收仍看得到，寫成名字＋白話原因、不露原始 JSON，同一隻敵人只一行", async () => {
  await load(
    world(
      { order: {}, turn: "", foes: {} },
      {
        refused: {
          "combat.foes.哥布林A": { wrote: '{"hp":"7/7"}', reason: "no_order" },
          "combat.foes.巨魔.hp": { wrote: "-3", reason: "missing" },
          "combat.foes.巨魔.conditions": { wrote: "中毒", reason: "missing" },
          // 同一則先被拒、後被移除的敵人：沒格可改，不列
          "combat.foes.豺狼D.hp": { wrote: "-3", reason: "bad_current" },
          "combat.turn": { wrote: "哥布林A", reason: "not_in_order" },
        },
      },
    ),
  );
  expect(host.querySelector(".state-combat")).toBeNull();
  const lines = [...host.querySelectorAll(".state-combat-idle .state-refused")].map((e) => e.textContent);
  expect(lines).toEqual([
    "⚠ 哥布林A 沒加上：還沒排行動順序",
    "⚠ 巨魔 沒更新：沒有這個敵人",
    "⚠ 輪到 哥布林A 沒設上：不在行動順序裡",
  ]);
});

it("HP 手改不合「現值/上限」不存、輸入框留著並提示格式；舊的非法值被拒時標記講格式", async () => {
  await load(world({ order: {}, turn: "", foes: {} }, { refused: { "characters.沃恩.hp": { wrote: "-4", reason: "bad_current" } } }));
  const badge = host.querySelector<HTMLButtonElement>("button.state-refused")!;
  expect(badge.textContent).toBe("⚠ HP 要寫成「現值/上限」，例 7/11");
  await act(async () => badge.click());
  act(() => ctl.changeEditValue("還很有精神"));
  expect(host.querySelector(".state-bar-field-form .state-refused")?.textContent).toBe("HP 要寫成「現值/上限」，例 7/11");
  await act(async () => ctl.save(["characters", "沃恩", "hp"], true, "還很有精神"));
  expect(invoke).not.toHaveBeenCalledWith("set_state_path", expect.anything());
  expect(host.querySelector(".state-bar-input")).not.toBeNull();
  act(() => ctl.changeEditValue("7/11"));
  expect(host.querySelector(".state-bar-field-form .state-refused")).toBeNull();
  await act(async () => ctl.save(["characters", "沃恩", "hp"], true, "7/11"));
  expect(invoke).toHaveBeenCalledWith("set_state_path", { worldId: "table", path: ["characters", "沃恩", "hp"], value: "7/11" });
});

it("狀態效果拒收點開整串編輯，空清單也能補，存檔走 set_panel_list", async () => {
  await load(
    world({ order: {}, turn: "", foes: {} }, { refused: { "characters.沃恩.conditions": { wrote: "流血" } } }),
  );
  const list = host.querySelector<HTMLElement>('.state-panel-list[aria-label="狀態效果"]')!;
  await act(async () => list.querySelector<HTMLButtonElement>("button.state-refused")!.click());
  const area = list.querySelector("textarea")!;
  expect(area.value).toBe("中毒");
  act(() => {
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(area, "中毒\n 流血 \n\n");
    area.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await act(async () => list.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
  expect(invoke).toHaveBeenCalledWith("set_panel_list", {
    worldId: "table",
    path: ["characters", "沃恩", "conditions"],
    items: ["中毒", "流血"],
  });
});

it("戰鬥結束只在這一幕剛由 GM 收場時顯示；換幕後不顯示", async () => {
  await load({ ...world({ order: {}, turn: "", foes: {} }, { combat_ended: 0 }), current_scene: 0 });
  expect(host.querySelector(".state-combat-ended")?.textContent).toBe("戰鬥結束，按「收回上一句」可復原");
  expect(summary()).not.toContain("戰鬥中");
  await load({ ...world({ order: {}, turn: "", foes: {} }, { combat_ended: 0 }), current_scene: 1 });
  expect(host.querySelector(".state-combat-ended")).toBeNull();
  await load(world({ order: {}, turn: "", foes: {} }, { changes: { "combat.order": "更新" } }));
  expect(host.querySelector(".state-combat-ended")).toBeNull();
});
