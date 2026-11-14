// @vitest-environment happy-dom
// VN 與聊天視圖之間的切換（vn-script-entry）：工具列切 VN 先過未存守門；VN 選單的「劇本」
// 不問已卸載編輯器留下的舊守門。測的是 controllers/ 的導覽 hook，行為屬 VN，測試跟著住這裡。
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => "new-id") }));

import {
  type WorkspaceNavigationController,
  useWorkspaceNavigationController,
} from "../../controllers/useWorkspaceNavigationController";

const characters = {
  player: null,
  metaOf: () => undefined,
  refresh: async () => [],
  reloadPlayer: async () => {},
  noteRemoved: async () => null,
  remove: async () => "cancelled" as const,
  removePlayer: async () => false,
};

let root: Root;
let nav: WorkspaceNavigationController;

function Probe() {
  nav = useWorkspaceNavigationController({ worldId: "w1", gmOnly: false, characters, onError: () => {} });
  return null;
}

beforeEach(async () => {
  root = createRoot(document.createElement("div"));
  await act(async () => root.render(createElement(Probe)));
});

afterEach(() => act(() => root.unmount()));

it("switching to VN from an editor asks about unsaved changes and stays when told to", async () => {
  await act(async () => nav.setMainView({ kind: "world" }));
  const guard = vi.fn(async () => false);
  nav.leaveGuard.current = guard;
  const go = vi.fn();
  await act(async () => nav.leaveEditorThen(go));
  expect(guard).toHaveBeenCalledTimes(1);
  expect(go).not.toHaveBeenCalled();
  guard.mockResolvedValueOnce(true);
  await act(async () => nav.leaveEditorThen(go));
  expect(go).toHaveBeenCalledTimes(1);
});

it("the script entry opens on the script tab without asking a stale guard", async () => {
  await act(async () => nav.setMainView({ kind: "character", id: "A" }));
  const stale = vi.fn(async () => false);
  nav.leaveGuard.current = stale;
  await act(async () => void nav.openWorldEditor(true));
  expect(stale).not.toHaveBeenCalled();
  expect(nav.mainView).toEqual({ kind: "world", fromVn: true });
});
