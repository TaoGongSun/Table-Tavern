// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, expect, it } from "vitest";
import { playing } from "../../../server/core/room-test-kit";
import type { ScenePanel } from "../../../server/core/types";
import { setLangOnly } from "../../i18n";
import { Hub } from "./fake-server";
import { RoomBody } from "./RoomBody";
import { useRoomSession } from "./useRoomSession";
import type { RoomEnv } from "./useRoom";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  setLangOnly("zh-TW");
});
const roots: Root[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) act(() => root.unmount());
  document.body.innerHTML = "";
});
async function until(check: () => unknown) {
  for (let i = 0; i < 100; i++) {
    await act(async () => void (await new Promise((resolve) => setTimeout(resolve, 1))));
    if (check()) return;
  }
  throw new Error("房間面板未更新");
}

function Client({ env, hub, id }: { env: RoomEnv; hub: Hub; id: string }) {
  const room = useRoomSession(
    env,
    { code: hub.room.code, joinSecret: hub.room.joinSecret, pairCode: null },
    hub.room.deviceId(id),
    id,
  );
  const desktop = id === "a";
  return (
    <div className={`app ${desktop ? "party-embedded" : "party-phone"}`}>
      <RoomBody
        room={room}
        origin="https://party.example"
        desktop={desktop ? { leaveLabel: "關房", onLeave: () => {}, inviteUrl: null } : undefined}
      />
    </div>
  );
}

it("桌面嵌入與手機共用三鍵抽屜，收到權威面板後唯讀顯示、點外關閉", async () => {
  const room = await playing();
  const hub = new Hub(room);
  hub.wrap = (fn) => act(fn);
  const clients = ["a", "b"].map((id) => {
    const el = document.body.appendChild(document.createElement("div"));
    const root = createRoot(el);
    roots.push(root);
    const env: RoomEnv = {
      href: `https://party.example/r/${room.code}`,
      storage: null,
      open: hub.open,
      fetch,
      userAgent: "test",
      replaceHref: () => {},
    };
    act(() => root.render(<Client env={env} hub={hub} id={id} />));
    return el;
  });
  await until(() => clients.every((el) => el.querySelector('[data-testid="story"]')));
  const generate = async (panel?: ScenePanel) => {
    await room.send(hub.sockets[0].id, "round.resolve", { round: room.s.round!.round });
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    await hub.exec("gen.part.begin", { dispatchId, part: 0, speaker: { kind: "gm" } });
    await hub.exec("gen.part.end", {
      dispatchId,
      part: 0,
      text: "大家往前走。",
      interrupted: false,
      state: { time: "夜", place: "酒館", present: "明" },
      ...(panel ? { panel } : {}),
    });
    await hub.exec("gen.done", { dispatchId, aborted: false });
  };
  await generate();
  await until(() => clients.every((el) => el.querySelector('[data-testid="scene-status"]')));
  const line = (el: Element) =>
    el.querySelector<HTMLButtonElement>('[data-testid="scene-status"]')!;
  const sheet = (el: Element) => el.querySelector<HTMLElement>('[data-testid="status-sheet"]');
  for (const el of clients) {
    act(() => line(el).click());
    expect(sheet(el)!.querySelectorAll("dt")).toHaveLength(3);
    expect(sheet(el)!.textContent).not.toMatch(/追蹤中|持有物/);
    act(() => sheet(el)!.querySelector("button")!.click());
    expect(sheet(el)).toBeNull();
  }
  await generate({
    characters: { 明: { status: "守門", items: ["鑰匙"] } },
    tracking: { goals: ["回家"], promises: ["守信"], clues: ["腳印"] },
  });
  for (const el of clients) {
    act(() => line(el).click());
    expect(sheet(el)!.textContent).toContain("鑰匙");
    expect(sheet(el)!.textContent).toContain("腳印");
    expect(sheet(el)!.querySelectorAll("input, textarea")).toHaveLength(0);
    expect(sheet(el)!.nextElementSibling?.classList.contains("room-dock")).toBe(true);
    act(() =>
      sheet(el)!
        .querySelector("h3")!
        .dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })),
    );
    expect(sheet(el)).not.toBeNull();
    act(() =>
      el
        .querySelector<HTMLElement>('[data-testid="story"]')!
        .dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })),
    );
    expect(sheet(el)).toBeNull();
    act(() => line(el).click());
    act(() => document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
    expect(sheet(el)).toBeNull();
    act(() => line(el).click());
    act(() => el.querySelector<HTMLButtonElement>('[data-testid="tab-members"]')!.click());
    act(() => el.querySelector<HTMLButtonElement>('[data-testid="tab-story"]')!.click());
    expect(sheet(el)).toBeNull();
    expect(line(el).getAttribute("aria-expanded")).toBe("false");
  }
});
