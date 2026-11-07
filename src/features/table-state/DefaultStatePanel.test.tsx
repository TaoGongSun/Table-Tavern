// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { setLang, t } from "../../i18n";
import { type WorldState } from "../../shared/contracts/backend-contracts";
import { StateBar } from "./StateBar";
import {
  useTableStateController,
  type TableStateController,
} from "./useTableStateController";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));
(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;
let host: HTMLDivElement, root: Root, ctl: TableStateController;
let state: WorldState;
const error = vi.fn();
function Probe({ room = false }: { room?: boolean }) {
  ctl = useTableStateController({ worldId: "table", onError: error });
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
      onAddPanelField={ctl.addPanelField}
      onRemovePanelField={ctl.removePanelField}
      onRemovePanelItem={ctl.removePanelItem}
      player={null}
      castCount={0}
      cast={[]}
      hideSceneSummary={room}
    />
  );
}
beforeEach(async () => {
  setLang("zh-TW");
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() });
  vi.mocked(invoke).mockReset();
  error.mockReset();
  state = {
    id: "table",
    name: "測試桌",
    player_card_id: null,
    model_bindings: {},
    current_scene: 0,
    catchup_summaries: {},
    scene_titles: {},
    scene_labels: {},
    table_kind: "chat",
    state: {
      table: { time: "夜", place: "酒館", present: "小林" },
      tree: {
        characters: {
          小林: {
            status: "警戒",
            items: { "0": "短劍", "1": "火把" },
            傷勢: "輕傷",
          },
        },
        tracking: {
          goals: {},
          promises: { "0": "回家" },
          clues: { "0": "足跡" },
        },
      },
    },
    mechanism: { default_panel: true },
  };
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "read_state"
      ? state
      : command === "branch_bindings"
        ? []
        : undefined,
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
const click = async (node: Element) =>
  act(async () => (node as HTMLElement).click());
const submit = async (form: HTMLFormElement) =>
  act(async () => {
    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
  });
function input(node: HTMLInputElement, value: string) {
  act(() => {
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )!.set!.call(node, value);
    node.dispatchEvent(new Event("input", { bubbles: true }));
  });
}
const cell = () =>
  host.querySelector<HTMLElement>('.state-panel-cell[aria-label="小林"]')!;

it("預設收合、固定鍵顯示標籤、角色格與三個追蹤欄；卡格式桌仍走樹", async () => {
  expect(host.querySelector("details.state-bar")?.hasAttribute("open")).toBe(
    false,
  );
  expect(host.querySelector("summary")?.textContent).toContain("夜");
  expect(cell().textContent).toContain("狀態警戒");
  expect(host.querySelector(".state-panel-tracking")?.textContent).toContain(
    "目標",
  );
  expect(host.querySelector(".state-panel-tracking")?.textContent).toContain(
    "承諾",
  );
  expect(host.querySelector(".state-panel-tracking")?.textContent).toContain(
    "線索",
  );
  expect(host.querySelector(".state-default-panel")?.textContent).not.toMatch(
    /characters|status|items|tracking|goals|promises|clues/,
  );
  expect(host.querySelectorAll(".state-panel-chip")).toHaveLength(4);
  await act(async () => ctl.hydrate(state.state, []));
  expect(host.querySelector(".state-default-panel")).toBeNull();
  expect(host.querySelector(".state-tree-branch")).not.toBeNull();
});

it("status 走原始路徑；chip 改值及刪項均整串寫入清單父路徑", async () => {
  await click(cell().querySelector(".state-bar-value")!);
  input(cell().querySelector("input")!, "休息");
  await submit(cell().querySelector("form")!);
  expect(invoke).toHaveBeenCalledWith("set_state_path", {
    worldId: "table",
    path: ["characters", "小林", "status"],
    value: "休息",
  });
  await click(cell().querySelector(".state-panel-chip .state-bar-value")!);
  input(cell().querySelector("input")!, "長劍");
  await submit(cell().querySelector("form")!);
  expect(invoke).toHaveBeenCalledWith("set_panel_list", {
    worldId: "table",
    path: ["characters", "小林", "items"],
    items: ["長劍", "火把"],
  });
  const remove = cell().querySelector<HTMLButtonElement>(
    ".state-panel-chip > button",
  )!;
  expect(remove.disabled).toBe(false);
  await click(remove);
  expect(invoke).toHaveBeenCalledWith("set_panel_list", {
    worldId: "table",
    path: ["characters", "小林", "items"],
    items: ["火把"],
  });
  expect(
    vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "set_state_path"),
  ).toHaveLength(1);
});

it("清空 chip 文字會刪該項，追蹤清單刪最後一項送空陣列", async () => {
  await click(cell().querySelectorAll(".state-panel-chip .state-bar-value")[1]);
  input(cell().querySelector("input")!, "");
  await submit(cell().querySelector("form")!);
  expect(invoke).toHaveBeenCalledWith("set_panel_list", {
    worldId: "table",
    path: ["characters", "小林", "items"],
    items: ["短劍"],
  });
  const clues = host.querySelector<HTMLElement>(
    '.state-panel-list[aria-label="線索"]',
  )!;
  await click(clues.querySelector(".state-bar-value")!);
  input(clues.querySelector("input")!, "新線索");
  await submit(clues.querySelector("form")!);
  expect(invoke).toHaveBeenCalledWith("set_panel_list", {
    worldId: "table",
    path: ["tracking", "clues"],
    items: ["新線索"],
  });
  await click(clues.querySelector(".state-panel-chip > button")!);
  expect(invoke).toHaveBeenCalledWith("set_panel_list", {
    worldId: "table",
    path: ["tracking", "clues"],
    items: [],
  });
  expect(
    vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "set_state_path"),
  ).toBe(false);
});

it("刪項失敗顯示錯誤並保留清單；取消編輯後仍可刪項", async () => {
  act(() => ctl.cancelEdit());
  vi.mocked(invoke).mockRejectedValueOnce("無法寫入清單");
  await click(cell().querySelector(".state-panel-chip > button")!);
  expect(error).toHaveBeenCalledWith("無法寫入清單");
  expect(cell().querySelectorAll(".state-panel-chip")).toHaveLength(2);
  await click(cell().querySelector(".state-panel-chip > button")!);
  expect(invoke).toHaveBeenLastCalledWith("branch_bindings", {
    worldId: "table",
  });
});

it("＋欄位拒絕保留鍵與重複名，新增與刪自訂欄各走專屬指令", async () => {
  await click(
    [...cell().querySelectorAll("button")].find(
      (el) => el.textContent === t("statePanelAddField"),
    )!,
  );
  const field = cell().querySelector<HTMLInputElement>(
    ".state-panel-add input",
  )!;
  for (const name of [
    "status",
    "items",
    "characters",
    "tracking",
    "goals",
    "promises",
    "clues",
    "傷勢",
  ]) {
    input(field, name);
    await submit(cell().querySelector(".state-panel-add")!);
    expect(cell().querySelector('[role="alert"]')?.textContent).toBe(
      t("statePanelInvalidName"),
    );
  }
  expect(
    vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "add_panel_field"),
  ).toBe(false);
  input(field, " 備註 ");
  await submit(cell().querySelector(".state-panel-add")!);
  expect(invoke).toHaveBeenCalledWith("add_panel_field", {
    worldId: "table",
    parentPath: ["characters", "小林"],
    name: "備註",
  });
  await click(cell().querySelector(".state-panel-custom > button")!);
  expect(invoke).toHaveBeenCalledWith("remove_panel_field", {
    worldId: "table",
    parentPath: ["characters", "小林"],
    name: "傷勢",
  });
});

it("追蹤區加欄位寫 tracking；指令失敗保留名稱並回報錯誤", async () => {
  const tracking = host.querySelector<HTMLElement>(
    '.state-panel-cell[aria-label="追蹤中"]',
  )!;
  await click(
    [...tracking.querySelectorAll("button")].find(
      (el) => el.textContent === t("statePanelAddField"),
    )!,
  );
  input(tracking.querySelector("input")!, "備忘");
  vi.mocked(invoke).mockRejectedValueOnce("無法寫入");
  await submit(tracking.querySelector("form")!);
  expect(error).toHaveBeenCalledWith("無法寫入");
  expect(tracking.querySelector("input")?.value).toBe("備忘");
  await submit(tracking.querySelector("form")!);
  expect(invoke).toHaveBeenCalledWith("add_panel_field", {
    worldId: "table",
    parentPath: ["tracking"],
    name: "備忘",
  });
});

it("房間模式收合列只畫入口，換桌 hydrate 同步預設面板旗標", async () => {
  await act(async () => root.render(<Probe room />));
  expect(host.querySelector("summary .state-bar-seg")).toBeNull();
  expect(host.querySelector("summary")?.textContent).toBe(t("stateBarEntry"));
  await act(async () =>
    ctl.hydrate(
      state.state,
      [],
      undefined,
      undefined,
      undefined,
      undefined,
      true,
    ),
  );
  expect(host.querySelector(".state-default-panel")).not.toBeNull();
});
