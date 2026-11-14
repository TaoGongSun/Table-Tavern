// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { useSprites } from "./useSprites";

let readAll: Record<string, unknown> = {};
const calls = vi.hoisted(() => [] as [string, Record<string, unknown>][]);
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async (command, args) => {
  calls.push([command, args]);
  return command === "vn_sprites_read_all" ? readAll : null;
}) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function Probe({ visible, active }: { visible: string[]; active: boolean }) {
  useSprites("w", visible, active);
  return null;
}
it("only reports visible characters while the stage is showing, never the whole cast", async () => {
  const root = createRoot(document.createElement("div"));
  calls.length = 0;
  await act(async () => root.render(createElement(Probe, { visible: ["a"], active: false })));
  expect(calls.filter(([name]) => name === "vn_sprites_fill")).toEqual([]);
  await act(async () => root.render(createElement(Probe, { visible: ["a"], active: true })));
  await act(async () => root.render(createElement(Probe, { visible: ["a"], active: true })));
  await act(async () => root.render(createElement(Probe, { visible: ["a", "b"], active: true })));
  expect(calls.filter(([name]) => name === "vn_sprites_fill").map(([, args]) => args.appeared)).toEqual([["a"], ["a", "b"]]);
  await act(async () => root.unmount());
});

it("a remount starts from the sprites read last time, so the stage shows them before the next cell", async () => {
  const seen: Record<string, unknown>[] = [];
  function Watch({ world }: { world: string }) {
    seen.push(useSprites(world));
    return null;
  }
  readAll = { a: { body: "data:a" } };
  const first = createRoot(document.createElement("div"));
  await act(async () => first.render(createElement(Watch, { world: "w" })));
  await act(async () => first.unmount());
  seen.length = 0;
  const again = createRoot(document.createElement("div"));
  await act(async () => again.render(createElement(Watch, { world: "w" })));
  expect(Object.keys(seen[0])).toEqual(["a"]);
  await act(async () => again.unmount());
  seen.length = 0;
  const other = createRoot(document.createElement("div"));
  readAll = {};
  await act(async () => other.render(createElement(Watch, { world: "w2" })));
  expect(seen[0]).toEqual({});
  await act(async () => other.unmount());
  readAll = {};
});
