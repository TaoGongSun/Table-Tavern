// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import type { RoomStatus } from "./RoomPanel";

const backend = vi.hoisted(() => ({ status: null as unknown, calls: [] as Array<[string, unknown]> }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: unknown) => {
    backend.calls.push([cmd, args]);
    return cmd === "room_status" ? backend.status : null;
  }),
}));

import { RoomPanel } from "./RoomPanel";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const idle: RoomStatus = {
  active: false,
  worldId: null,
  server: "https://rooms.example.test",
  roomCode: null,
  inviteUrl: null,
  pairCode: null,
  payerLink: null,
  online: false,
  keepAwake: null,
  modelLabel: null,
  lastError: null,
};

let host: HTMLDivElement;
let root: Root;

async function render(worldId = "w1") {
  await act(async () => {
    root.render(<RoomPanel worldId={worldId} onClose={() => {}} />);
  });
  await act(async () => {});
}

function button(label: string) {
  return [...host.ownerDocument.querySelectorAll("button")].find((b) => b.textContent === label) as HTMLButtonElement | undefined;
}

beforeEach(() => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  backend.calls = [];
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("RoomPanel", () => {
  it("沒有房間時給伺服器網址欄與進入鈕，按下帶這桌與網址", async () => {
    backend.status = idle;
    await render();
    const input = host.ownerDocument.querySelector<HTMLInputElement>("input.room-value");
    expect(input?.value).toBe("https://rooms.example.test");
    expect(host.ownerDocument.querySelector('[role="note"]')?.textContent).toBe(t("roomRisk"));
    await act(async () => button(t("roomEnter"))!.click());
    expect(backend.calls).toContainEqual(["room_enter", { worldId: "w1", server: "https://rooms.example.test" }]);
  });

  it("房間模式中顯示房間碼、兩條連結、配對碼、模型與最近錯誤", async () => {
    backend.status = {
      ...idle,
      active: true,
      worldId: "w1",
      roomCode: "ABCD2345",
      inviteUrl: "https://rooms.example.test/r/ABCD2345#k=secret",
      pairCode: "WXYZ-2345",
      payerLink: "https://rooms.example.test/r/ABCD2345#k=secret&p=WXYZ2345",
      online: true,
      keepAwake: true,
      modelLabel: "claude-sonnet",
      lastError: "GM 額度用完",
    } satisfies RoomStatus;
    await render();
    const text = host.ownerDocument.body.textContent ?? "";
    for (const expected of ["ABCD2345", "WXYZ-2345", "claude-sonnet", "GM 額度用完", t("roomOnline"), t("roomKeepAwakeOn")]) {
      expect(text).toContain(expected);
    }
    const links = [...host.ownerDocument.querySelectorAll<HTMLInputElement>("input.room-value")].map((i) => i.value);
    expect(links).toEqual([
      "https://rooms.example.test/r/ABCD2345#k=secret",
      "https://rooms.example.test/r/ABCD2345#k=secret&p=WXYZ2345",
    ]);
    await act(async () => button(t("roomResetHost"))!.click());
    await act(async () => button(t("roomExit"))!.click());
    expect(backend.calls.map(([cmd]) => cmd)).toEqual(expect.arrayContaining(["room_pair_code", "room_exit"]));
  });

  it("別桌在房間模式時提示並停用進入鈕", async () => {
    backend.status = { ...idle, active: true, worldId: "other" };
    await render("w1");
    expect(host.ownerDocument.body.textContent).toContain(t("roomOtherTable"));
    expect(button(t("roomEnter"))!.disabled).toBe(true);
  });
});
