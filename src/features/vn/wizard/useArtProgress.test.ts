// @vitest-environment happy-dom
// 標題畫面的立繪進度：重開 app 後還缺定稿、也沒在排就再排一次自動定稿；已在排或都齊了不重排。
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, expect, it, vi } from "vitest";
import type { SpriteSlotStatus } from "../../../shared/contracts/backend-contracts";
import { useArtProgress } from "./useArtProgress";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const backend = {
  calls: [] as string[],
  sprites: {} as Record<string, unknown>,
  statuses: [] as SpriteSlotStatus[],
};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    backend.calls.push(command);
    if (command === "list_characters")
      return ["a", "b", "c"].map((id) => ({ id, name: id, archived: false, stage: false }));
    if (command === "vn_sprites_read_all") return backend.sprites;
    if (command === "vn_sprites_status") return backend.statuses;
    return null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

function Probe() {
  const { progress } = useArtProgress("W");
  return createElement("span", null, progress ? `${progress.ready}/${progress.total}` : "");
}

async function mount() {
  const host = document.createElement("div");
  await act(async () => createRoot(host).render(createElement(Probe)));
  await act(async () => new Promise((resolve) => setTimeout(resolve, 0)));
  return host;
}

const prepares = () => backend.calls.filter((command) => command === "vn_sprites_prepare").length;

beforeEach(() => {
  backend.calls = [];
  backend.sprites = { a: {} };
  backend.statuses = [];
});

it("resumes the automatic base when sprites are missing and nothing is queued", async () => {
  const host = await mount();
  expect(host.textContent).toBe("1/3");
  expect(prepares()).toBe(1);
});

it("does not re-queue while a base is still queued or running, nor when all are ready", async () => {
  backend.statuses = [{ worldId: "W", characterId: "b", variant: null, expression: "base", state: "running", reason: null }];
  await mount();
  expect(prepares()).toBe(0);
  backend.statuses = [];
  backend.sprites = { a: {}, b: {}, c: {} };
  await mount();
  expect(prepares()).toBe(0);
});
