// @vitest-environment happy-dom
// 房間骰鈕（room-player-dice）：手機與房主都擲得到、空框一鍵送出、其他人立刻看到、被拒不沉默。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, expect, it } from "vitest";
import { playing } from "../../../server/core/room-test-kit";
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
  throw new Error(`等不到：${check}`);
}
function Client({ env, hub, id }: { env: RoomEnv; hub: Hub; id: string }) {
  const room = useRoomSession(
    env,
    { code: hub.room.code, joinSecret: hub.room.joinSecret, pairCode: null },
    hub.room.deviceId(id),
    id,
  );
  return (
    <div className="app party-phone">
      <RoomBody room={room} origin="https://party.example" />
    </div>
  );
}
async function table(diceEnabled = true) {
  const room = await playing();
  const hub = new Hub(room);
  hub.wrap = (fn) => act(fn);
  const [a, b] = ["a", "b"].map((id) => {
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
      retryDelays: [0],
    };
    act(() => root.render(<Client env={env} hub={hub} id={id} />));
    return el;
  });
  await until(() => a.querySelector('[data-testid="action-box"]'));
  await until(() => b.querySelector('[data-testid="action-box"]'));
  await hub.exec("status", { keepAwake: null, busy: false, diceEnabled });
  await act(async () => {});
  return { room, hub, a, b };
}
const rollButton = (el: Element) =>
  el.querySelector<HTMLButtonElement>(".action-dice .dice-controls button")!;
const customRoll = async (el: Element) => {
  act(() => rollButton(el).click());
  const custom = [...el.querySelectorAll<HTMLButtonElement>(".dice-custom button")].at(-1)!;
  act(() => custom.click());
};
const typeAction = (el: Element, value: string) =>
  act(() => {
    const input = el.querySelector<HTMLTextAreaElement>('[data-testid="action-text"]')!;
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(
      input,
      value,
    );
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });

it("空框擲骰：一鍵送出、其他人立刻看到，沒有空白的已送出列；收回後提示擲果還在", async () => {
  const { room, a, b } = await table();
  await customRoll(b);
  await until(() => a.querySelector('[data-testid="story"]')!.textContent?.includes("1d20: ["));
  const entry = room.s.round!.roster.find((e) => e.memberId === "m2")!;
  expect(entry).toMatchObject({ status: "submitted", text: "" });
  expect(a.querySelector('[data-testid="submitted"]')).toBeNull();
  await until(() => b.querySelector(".action-hint")?.textContent?.includes("已送出"));
  expect(b.querySelector('[data-testid="story"]')!.textContent).toContain("角色2 1d20");
  const retract = [...b.querySelectorAll("button")].find((x) => x.textContent === "收回修改")!;
  act(() => retract.click());
  await until(() => b.querySelector('[data-testid="roll-kept-note"]'));
});

it("房主有座位時同一顆；有字不送，字留在框裡", async () => {
  const { room, a } = await table();
  typeAction(a, "我拔劍");
  await customRoll(a);
  await until(() => room.s.pendingRolls?.length === 1);
  expect(room.s.round!.roster[0].status).not.toBe("submitted");
  expect(a.querySelector<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe("我拔劍");
  await until(() => a.querySelector('[data-testid="roll-kept-note"]'));
});

it("建議取自最新 GM 文字：自己的當主鈕，擲過換下一個，自己的擲完退回通用鈕、不落到別人的；擲過的標 ✓ 還能再擲", async () => {
  const { room, hub, b } = await table();
  await room.send(hub.sockets[0].id, "round.resolve", { round: room.s.round!.round });
  await room.finishGen("角色1請擲1d20+1，角色2請擲1d20+3，傷害2d6。");
  hub.flush();
  await until(() => rollButton(b).textContent?.includes("1d20+3"));
  expect(b.querySelector(".dice-more")).not.toBeNull();
  typeAction(b, "再想想"); // 有字：不會一起送出，才擲得了下一顆
  act(() => b.querySelector<HTMLButtonElement>('[data-testid="action-tuck"]')!.click());
  act(() => rollButton(b).click()); // 收起時骰鈕還在
  await until(() => rollButton(b).textContent?.includes("2d6"));
  expect(room.s.pendingRolls![0].text).toMatch(/^角色2 1d20\+3: /);
  act(() => rollButton(b).click());
  await until(() => !b.querySelector(".dice-more"));
  expect(rollButton(b).textContent).toContain("擲骰");
  expect(rollButton(b).textContent).not.toContain("1d20+1");
  act(() => rollButton(b).click());
  const options = [...b.querySelectorAll<HTMLButtonElement>(".dice-option")];
  expect(options.map((x) => x.textContent)).toEqual(["✓ 擲 1d20+3", "✓ 擲 2d6", "擲 1d20+1"]);
  act(() => options[0].click());
  await until(() => room.s.pendingRolls?.length === 3);
});

it("擲果行總和加粗（切最後一個等號）", async () => {
  const { b } = await table();
  typeAction(b, "想一下");
  await customRoll(b);
  await until(() => b.querySelector(".part-dice strong"));
  expect(b.querySelector(".part-dice strong")!.textContent).toMatch(/^= \d+$/);
  expect(b.querySelector(".part-dice .part-text")!.textContent).toMatch(
    /^角色2 1d20: \[\d+\] = \d+$/,
  );
});

it("停用理由：沒開擲骰、GM 生成中都按了才講，不送指令", async () => {
  const { hub, room, a, b } = await table(false);
  act(() => rollButton(b).click());
  expect(b.querySelector('.action-dice [role="status"]')?.textContent).toBe("房主還沒開擲骰");
  act(() => rollButton(a).click());
  expect(a.querySelector('.action-dice [role="status"]')?.textContent).toBe(
    "這桌還沒開擲骰（桌設定）",
  );
  await hub.exec("status", { keepAwake: null, busy: false, diceEnabled: true });
  await room.send(hub.sockets[0].id, "round.resolve", { round: room.s.round!.round });
  hub.flush();
  act(() => rollButton(b).click());
  expect(b.querySelector('.action-dice [role="status"]')?.textContent).toBe("等 GM 說完再擲");
  expect(room.s.pendingRolls ?? []).toEqual([]);
});

it("被拒不沉默：限頻顯示錯誤；重連後擲果還在", async () => {
  const { room, hub, b } = await table();
  typeAction(b, "想一下");
  for (let i = 0; i < 5; i++) {
    await customRoll(b);
    await until(() => room.s.pendingRolls?.length === i + 1);
  }
  await customRoll(b);
  await until(() => b.querySelector('[data-testid="action-error"]'));
  expect(b.querySelector('[data-testid="action-error"]')!.textContent).toBe(
    "這次擲骰沒成功，請再試一次",
  );
  const count = () =>
    (b.querySelector('[data-testid="story"]')!.textContent ?? "").split("1d20: [").length - 1;
  expect(count()).toBe(5);
  act(() => hub.sockets[1].close(1006));
  await until(() => hub.sockets.length > 2 && count() === 5);
});
