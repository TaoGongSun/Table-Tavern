// @vitest-environment happy-dom
// 本桌人名表：present 不變、名冊在世界書編輯畫面改了，回到遊戲（refreshKey 變）就重讀；同桌重讀失敗只用卡名。

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

const { useKnownPresentNames } = await import("./present-glyphs");

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLDivElement;
let root: Root;
let seen: string[] = [];

function Probe({ refreshKey }: { refreshKey: string }) {
  seen = useKnownPresentNames("w1", ["贺铮"], "林远", "瀋鬱", refreshKey);
  return null;
}

async function render(refreshKey: string) {
  await act(async () => {
    root.render(<Probe refreshKey={refreshKey} />);
  });
  await act(async () => {
    await Promise.resolve();
  });
}

beforeEach(() => {
  invoke.mockReset();
  host = document.createElement("div");
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
});

const person = (title: string, disabled = false) => ({ title, is_person: true, disabled });

it("present 不變、名冊改了：換 refreshKey 重讀，新增、停用、改名都跟上", async () => {
  invoke.mockResolvedValue([person("沈郁"), { title: "總綱", is_person: false }]);
  await render("true\u00000");
  expect(seen).toEqual(["贺铮", "林远", "沈郁"]);

  invoke.mockResolvedValue([person("沈郁"), person("沈鬱"), person("阿古拉", true)]);
  await render("true\u00000");
  expect(seen).toEqual(["贺铮", "林远", "沈郁"]);
  await render("false\u00000");
  await render("true\u00000");
  expect(seen).toEqual(["贺铮", "林远", "沈郁", "沈鬱"]);
  expect(invoke).toHaveBeenLastCalledWith("read_worldbook", { worldId: "w1" });
});

it("同桌重讀失敗就只用卡名與玩家名，不留舊條目", async () => {
  invoke.mockResolvedValue([person("沈郁")]);
  await render("a");
  expect(seen).toContain("沈郁");
  invoke.mockRejectedValue(new Error("讀不到"));
  await render("b");
  expect(seen).toEqual(["贺铮", "林远"]);
});
