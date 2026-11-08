// @vitest-environment happy-dom
/// <reference types="node" />
// 手機客戶端整合測試：真的伺服器核心（room-test-kit）當假伺服器，兩支「手機」各自渲染 App，透過假 WebSocket 連進同一房。
import { readFileSync } from "node:fs";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { MemoryStorage } from "@web/features/openrouter/memory-storage";
import { CARDS, Room, STAGE } from "../../../server/core/room-test-kit";
import { ChatPanel } from "../chat/ChatPanel";
import { seatBlurb } from "../join/seat-blurb";
import { SeatPicker } from "../join/SeatPicker";
import type { Act } from "./useRoom";
import type { RoomView } from "./room-state";
import { receiveUpload } from "../../../server/core/upload";
import App from "../../App";
import { applyLang } from "../../i18n";
import { Hub, type FakeSocket } from "./fake-server";
import type { RoomEnv } from "./useRoom";

// happy-dom 沒有版面與圖片解碼：裁切元件一掛上就回報預設框（整張），其餘照真的流程
vi.mock("react-easy-crop", async () => {
  const { useEffect } = await import("react");
  return {
    default: ({ onCropComplete }: { onCropComplete: (a: object, p: object) => void }) => {
      useEffect(
        () =>
          onCropComplete(
            { x: 0, y: 0, width: 100, height: 100 },
            { x: 0, y: 0, width: 1, height: 1 },
          ),
        [],
      );
      return null;
    },
  };
});

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  applyLang("zh-TW");
});

const mounted: Root[] = [];
afterEach(() => {
  vi.unstubAllGlobals();
  for (const r of mounted.splice(0)) act(() => r.unmount());
  document.body.innerHTML = "";
});

/** 等伺服器（WebCrypto、序列化佇列）與 React 都跑完 */
async function until(check: () => unknown, tries = 50) {
  for (let i = 0; i < tries; i++) {
    await act(async () => void (await new Promise((r) => setTimeout(r, 1))));
    if (check()) return;
  }
  throw new Error(`等不到：${check}`);
}

function phone(hub: Hub, hash: string, extra: Partial<RoomEnv> = {}) {
  const el = document.body.appendChild(document.createElement("div"));
  const root = createRoot(el);
  mounted.push(root);
  const hrefs: string[] = [];
  const mine: FakeSocket[] = [];
  const open = () => {
    const s = hub.open();
    mine.push(s);
    return s;
  };
  const env = {
    href: `https://party.example/r/${hub.room.code}#${hash}`,
    storage: new MemoryStorage(),
    open,
    fetch,
    userAgent: "Mozilla/5.0",
    replaceHref: (h: string) => void hrefs.push(h),
    retryDelays: [0],
    ...extra,
  };
  act(() => root.render(<App env={env} lang="zh-TW" />));
  const q = <T extends Element = HTMLElement>(sel: string) => el.querySelector<T>(sel);
  const buttons = () => [...el.querySelectorAll("button")];
  // 結算鈕會帶「（N 人還沒送）」：找不到完全相同的就認開頭
  const button = (label: string) =>
    buttons().find((b) => b.textContent === label) ??
    buttons().find((b) => b.textContent?.startsWith(`${label}（`)) ??
    null;
  return {
    el,
    env,
    hrefs,
    q,
    button,
    text: () => el.textContent ?? "",
    click: (label: string) => act(() => button(label)!.click()),
    tab: (k: string) => act(() => q<HTMLButtonElement>(`[data-testid="tab-${k}"]`)!.click()),
    type: (sel: string, value: string) =>
      act(() => {
        const input = q<HTMLInputElement | HTMLTextAreaElement>(sel)!;
        Object.getOwnPropertyDescriptor(Object.getPrototypeOf(input), "value")!.set!.call(
          input,
          value,
        );
        input.dispatchEvent(new Event("input", { bubbles: true }));
      }),
    /** 這支手機目前（最新）的連線 */
    socket: () => mine.filter((s) => s.readyState !== 3).at(-1) as FakeSocket,
  };
}
type Phone = ReturnType<typeof phone>;

async function joinAndSit(p: Phone, nickname: string, card: string) {
  p.type('[data-testid="nickname"]', nickname);
  await act(async () => p.q<HTMLFormElement>("form")!.requestSubmit());
  await until(() => p.q('[data-testid="seat-picker"]'));
  const row = [...p.el.querySelectorAll(".seat-card")].find((r) => r.textContent?.includes(card))!;
  act(() => row.querySelector("button")!.click());
  await until(() => p.q('[data-testid="action-box"]'));
}

/** 標準開局：A 掃出額度者 QR（房主＋出額度者）、B 用一般邀請；兩人入座、A 開房、執行端串流跑完開場 → 第 2 回合行動中 */
async function table(bEnv: Partial<RoomEnv> = {}) {
  const room = await Room.create();
  await room.exec();
  await room.send("x1", "roster.update", { cards: [...CARDS, STAGE] });
  const hub = new Hub(room);
  hub.wrap = (fn) => act(fn);
  const A = phone(hub, `k=${room.joinSecret}&p=${await room.pairCode(true)}`);
  await joinAndSit(A, "阿明", "角色1");
  const B = phone(hub, `k=${room.joinSecret}`, bEnv);
  await joinAndSit(B, "小華", "角色2");
  A.click("開始");
  await until(() => room.s.round?.dispatchId);
  const dispatchId = room.dispatchId();
  await hub.exec("dispatch.ack", { dispatchId });
  await hub.exec("gen.part.begin", { dispatchId, part: 0, speaker: { kind: "gm" } });
  await hub.exec("gen.delta", { dispatchId, part: 0, text: "風在吹" });
  await until(() => B.q('[data-testid="streaming"]')?.textContent?.includes("風在吹"));
  // 回覆尾端的狀態欄圍欄串流中就截掉（同桌面版 streamDisplayText）
  await hub.exec("gen.delta", { dispatchId, part: 0, text: "。\n\n```state\ntime: 夜" });
  await until(() => B.q('[data-testid="streaming"]')?.textContent?.includes("風在吹。"));
  expect(B.q('[data-testid="streaming"]')!.textContent).not.toContain("```");
  expect(B.q('[data-testid="scene-status"]')).toBeNull();
  await hub.exec("gen.part.end", {
    dispatchId,
    part: 0,
    text: "風在吹，酒館門開了。",
    interrupted: false,
    state: { time: "夜", place: "", present: "阿明" },
  });
  await until(
    () =>
      !B.q('[data-testid="streaming"]') &&
      B.q('[data-testid="scene-status"]')?.textContent?.includes("夜"),
  );
  const line = B.q('[data-testid="scene-status"]')!;
  expect(line.textContent).toContain("時間 夜");
  expect(line.textContent).toContain("在場 阿明");
  expect(line.textContent).not.toContain("地點");
  expect(line.textContent).not.toContain("：");
  expect(line.querySelector(".scene-status-item")?.getAttribute("title")).toBe("時間 夜");
  B.tab("members");
  expect(B.q('[data-testid="scene-status"]')).toBeNull();
  B.tab("story");
  await hub.exec("gen.done", { dispatchId, aborted: false });
  await until(() => A.text().includes("輪到大家行動") && B.text().includes("輪到大家行動"));
  return { room, hub, A, B, a: room.s.members[0].id, b: room.s.members[1].id };
}

describe("手機客戶端對真伺服器核心", () => {
  it("進房後品牌列讓給劇情：房號、語系、劇情字級在名單頁；字級存在這支手機、重開照舊", async () => {
    const { hub, A, room } = await table();
    expect(A.q(".topbar")).toBeNull();
    const app = () => A.q(".app")!;
    expect(app().style.getPropertyValue("--story-size")).toBe("17px");
    A.tab("members");
    expect(A.q('[data-testid="display-settings"]')!.textContent).toContain(`房號 ${room.code}`);
    act(() => A.q<HTMLButtonElement>('[data-testid="story-size-xl"]')!.click());
    expect(app().style.getPropertyValue("--story-size")).toBe("22px");
    const again = phone(hub, `k=${room.joinSecret}`, { storage: A.env.storage });
    await until(() => again.q('[data-testid="round-bar"]'));
    expect(again.q(".app")!.style.getPropertyValue("--story-size")).toBe("22px");
  });

  it("保留給電腦的玩家卡：手機選角清單不列", async () => {
    const room = await Room.create();
    await room.exec();
    await room.send("x1", "roster.update", { cards: CARDS });
    await room.send("x1", "seat.reserve", { cardId: "c1", deviceId: room.deviceId("pc") });
    const hub = new Hub(room);
    hub.wrap = (fn) => act(fn);
    const B = phone(hub, `k=${room.joinSecret}`);
    B.type('[data-testid="nickname"]', "小華");
    await act(async () => B.q<HTMLFormElement>("form")!.requestSubmit());
    await until(() => B.q('[data-testid="seat-picker"]'));
    expect(B.text()).not.toContain("角色1");
    expect(B.text()).toContain("角色2");
  });

  it("大廳裡掛上行動格不會被當成催促：不震動、不搶焦點", async () => {
    const vibrate = vi.fn();
    Object.defineProperty(navigator, "vibrate", { value: vibrate, configurable: true });
    const room = await Room.create();
    await room.exec();
    await room.send("x1", "roster.update", { cards: [...CARDS, STAGE] });
    const hub = new Hub(room);
    hub.wrap = (fn) => act(fn);
    const A = phone(hub, `k=${room.joinSecret}&p=${await room.pairCode(true)}`);
    await joinAndSit(A, "阿明", "角色1");
    await until(() => A.q('[data-testid="action-box"]'));
    expect(vibrate).not.toHaveBeenCalled();
    expect(document.activeElement).not.toBe(A.q('[data-testid="action-text"]'));
    Reflect.deleteProperty(navigator, "vibrate");
  });

  it("空房：沒人入座時開始鈕停用並說原因；自建角色名字預填暱稱、執行端建卡後入座就能開始；狀態列看得到自己；房主暫停跟離線分開顯示", async () => {
    const room = await Room.create();
    await room.exec();
    const hub = new Hub(room);
    hub.wrap = (fn) => act(fn);
    const A = phone(hub, `k=${room.joinSecret}&p=${await room.pairCode(true)}`);
    A.type('[data-testid="nickname"]', "阿明");
    await act(async () => A.q<HTMLFormElement>("form")!.requestSubmit());
    await until(() => A.q('[data-testid="seat-create"]'));
    expect((A.button("開始") as HTMLButtonElement).disabled).toBe(true);
    expect(A.q('[data-testid="start-blocked"]')!.textContent).toBe("至少一位玩家入座才能開始");
    A.click("建立角色");
    expect(A.q<HTMLInputElement>('[data-testid="card-name"]')!.value).toBe("阿明");
    await act(async () => A.q<HTMLFormElement>('[data-testid="card-sheet"]')!.requestSubmit());
    await until(() => room.s.jobs.length > 0);
    const jobId = room.s.jobs[0].job.jobId;
    const card = {
      cardId: "mine",
      name: "阿明",
      color: null,
      avatar: null,
      blurb: "",
      playable: true,
      hidden: false,
    };
    await hub.exec("roster.update", { cards: [card] });
    await hub.exec("job.result", { jobId, ok: true, result: { cardId: "mine" } });
    await until(() => A.q('[data-testid="action-box"]'));
    expect((A.button("開始") as HTMLButtonElement).disabled).toBe(false);
    A.click("開始");
    await until(() => room.s.round?.dispatchId);
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    await hub.exec("gen.part.begin", { dispatchId, part: 0, speaker: { kind: "gm" } });
    await hub.exec("gen.part.end", { dispatchId, part: 0, text: "開場。", interrupted: false });
    await hub.exec("gen.done", { dispatchId, aborted: false });
    // 自己在狀態列，不是按鈕，沒有催促
    await until(() => A.q('[data-testid="seat-m1"]'));
    expect(A.q('[data-testid="round-waiting"]')).toBeNull();
    expect(A.q('[data-testid="seat-m1"]')!.textContent).toContain("阿明");
    expect(A.q('[data-testid="seat-m1"]')!.closest("button")).toBeNull();
    expect(A.button("結算這回合")!.textContent).toBe("結算這回合（1 人還沒送）");
    await hub.exec("status", { keepAwake: null, busy: false, paused: true });
    await act(async () => {
      await room.disconnect("x1");
      await hub.flush();
    });
    await until(() => A.text().includes("房主暫時關房，等他重開"));
  });

  it("加入、選角、開場串流；一回合：草稿不公開、送出即公開、跳過、全員到齊自動結算", async () => {
    const { room, A, B, a } = await table();
    // 加入時配對碼一次性：成功後從網址拿掉；這台裝置記得加入過、下次不再問暱稱
    expect(A.hrefs.at(-1)).toBe(`https://party.example/r/${room.code}#k=${room.joinSecret}`);
    expect(A.env.storage!.getItem("tt-party:rooms")).toContain("阿明");
    expect(room.s.hostId).toBe(a);
    expect(room.s.payerId).toBe(a);
    expect(A.q('[data-testid="tab-host"]') && A.q('[data-testid="tab-payer"]')).toBeTruthy();
    expect(B.q('[data-testid="tab-host"]') ?? B.q('[data-testid="tab-payer"]')).toBeNull();

    A.type('[data-testid="action-text"]', "我拔劍");
    await until(() => room.s.drafts[a] === "我拔劍");
    B.tab("members");
    await until(() => B.q(`[data-testid="member-${a}"]`)?.textContent?.includes("打字中"));
    expect(B.text()).not.toContain("我拔劍"); // 草稿不公開
    B.tab("story");

    A.click("送出");
    await until(() => B.q('[data-testid="submitted"]')?.textContent?.includes("我拔劍")); // 結算前就公開（裁決 9）
    expect(A.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe("");
    expect(A.button("收回修改")).toBeTruthy();
    expect(A.q('[data-testid="submitted-note"]')!.textContent).toBe(
      "已送出：我拔劍（結算前可以收回修改）",
    ); // 輸入框清空了，寫出送了什麼

    B.click("交給 GM");
    await until(() => room.s.round?.phase === "queued");
    await until(() => A.text().includes("GM 生成中…") && B.text().includes("GM 生成中…"));
    // 結算後收不回：不留一顆灰著的「收回修改」，照常給停用的跳過／送出
    expect(A.button("收回修改")).toBeNull();
    expect((A.button("送出") as HTMLButtonElement).disabled).toBe(true);
    expect(room.events("round.resolved").at(-1)!.payload.by).toBe("all");
    expect(B.q('[data-testid="story"]')!.textContent).toContain("我拔劍"); // 結算後進故事串
    expect(A.q('[data-testid="story"]')!.textContent).toContain("角色2這回合交給 GM 安排");
  });

  it("換裝置認領：新裝置按「這是我的？」、房主名單卡片確認；拒絕時玩家看得到、不卡住", async () => {
    const { room, hub, A } = await table();
    await room.join("z", "格羅");
    await room.send("z", "seat.claim", { cardId: "c3" });
    await room.disconnect("z");
    const C = phone(hub, `k=${room.joinSecret}`);
    C.type('[data-testid="nickname"]', "格羅");
    await act(async () => C.q<HTMLFormElement>("form")!.requestSubmit());
    await until(() => C.q('[data-testid="seat-mine-c3"]'));
    expect(C.text()).toContain("已有人選・離線");
    expect(C.text()).toContain("對方也叫格羅");
    expect(C.q('[data-testid="seat-mine-c1"]')).toBeNull(); // 房主的座位不給認領

    act(() => C.q<HTMLButtonElement>('[data-testid="seat-mine-c3"]')!.click());
    await until(() => C.q('[data-testid="seat-claim-waiting"]'));
    expect(C.q('[data-testid="seat-claim-waiting"]')!.textContent).toBe(
      "等房主確認；也可以先選別的角色",
    );
    await until(() => A.q('[data-testid="members-badge"]')?.textContent === "1");
    expect(A.text()).toContain("格羅想接回角色3，到名單確認"); // 停在故事頁也看得到
    A.tab("members");
    expect(A.text()).toContain("新加入的「格羅」說「角色3」是他的，要交給他嗎？");
    expect(A.text()).toContain("兩邊暱稱相同");

    A.click("不是");
    await until(() => C.text().includes("房主說這不是你的座位。"));
    expect(A.q('[data-testid="members-badge"]')).toBeNull();

    act(() => C.q<HTMLButtonElement>('[data-testid="seat-mine-c3"]')!.click());
    await until(() => A.button("是，交給他"));
    A.click("是，交給他");
    await until(() => C.q('[data-testid="action-box"]'));
    expect(room.s.members.filter((m) => m.nickname === "格羅")).toHaveLength(1);
    await until(() => !A.button("是，交給他"));
  });

  it("房主結算；回合已往下走時送出被拒，草稿原樣保留", async () => {
    const { room, hub, A, B } = await table();
    B.socket().held = true; // B 暫時收不到：畫面停在第 2 回合
    A.click("結算這回合");
    await until(() => room.s.round?.phase === "queued");
    await room.finishGen("第二段");
    hub.flush();
    expect(room.s.round!.round).toBe(3);
    expect(A.q('[data-testid="story"]')!.textContent).toContain("〈角色2〉這回合觀望");

    B.type('[data-testid="action-text"]', "我躲到吧台後面");
    B.click("送出");
    await until(() =>
      room.out[B.socket().id].some((f) => f.t === "err" && f.p.code === "E_ROUND_STALE"),
    );
    B.socket().held = false;
    hub.flush();
    await until(() => B.q('[data-testid="action-error"]'));
    expect(B.q('[data-testid="action-error"]')!.textContent).toBe("回合已經往下走了，你的字還留著");
    expect(B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe("我躲到吧台後面");
    expect(B.text()).toContain("第 1 幕 · 第 3 回合");
  });

  it("斷線後重連帶 lastSeq，只補漏掉的事件", async () => {
    const { room, A, B } = await table();
    const old = B.socket();
    old.held = true;
    A.type('[data-testid="action-text"]', "我推開門");
    A.click("送出");
    await until(() => room.events("action.submitted").length === 1);
    act(() => old.close(1006)); // 網路斷了：沒收到的訊框就此遺失
    await until(
      () =>
        B.socket() &&
        B.socket() !== old &&
        B.socket().readyState === 1 &&
        B.text().includes("我推開門"),
    );
    const hello = B.socket().sent[0];
    expect(hello.t).toBe("hello");
    expect(hello.p.lastSeq).toBeGreaterThan(0);
    const welcome = room.out[B.socket().id].find((f) => f.t === "welcome")!;
    expect(welcome.p.snapshot).toBeUndefined(); // 補事件路徑，不是整份快照
    expect(B.q('[data-testid="submitted"]')!.textContent).toContain("我推開門");
  });

  it("這一幕快滿時只提醒房主，換幕完成後消失", async () => {
    const { room, hub, A, B } = await table();
    await hub.exec("status", { keepAwake: null, busy: false, capacityHint: true });
    await until(() => A.q('[data-testid="capacity-note"]'));
    expect(A.q('[data-testid="capacity-note"]')!.textContent).toBe(
      "這一幕快滿了，再玩幾回合 GM 就送不出去，建議換幕",
    );
    expect(B.q('[data-testid="capacity-note"]')).toBeNull();

    A.click("換幕 ▾"); // 回合列選單（room-scene-advance-bar）
    const endScene = () =>
      [...A.el.querySelectorAll<HTMLButtonElement>(".menu .menu-item")].find((b) =>
        b.textContent?.startsWith("結束本幕"),
      );
    await until(() => endScene());
    act(() => endScene()!.click());
    await until(() => room.s.status === "transition");
    const { jobId } = room.frames("x1", "job.scene_advance")[0].p;
    await hub.exec("job.result", { jobId, ok: true, result: { summary: "前情：大家進了酒館。" } });
    await until(() => !A.q('[data-testid="capacity-note"]'));
    expect(B.q('[data-testid="capacity-note"]')).toBeNull();
  });

  it("換幕：已送出的行動退回草稿、新幕開場帶前情摘要；房主移出成員、關房後畫面唯讀", async () => {
    const { room, hub, A, B } = await table();
    B.type('[data-testid="action-text"]', "我想走");
    B.click("送出");
    await until(() => room.events("action.submitted").length === 1);
    const round = room.s.round!.round;
    A.click("換幕 ▾"); // 故事頁回合列：一項選單就是確認，不帶下一幕路線
    const endScene = () =>
      [...A.el.querySelectorAll<HTMLButtonElement>(".menu .menu-item")].find((b) =>
        b.textContent?.startsWith("結束本幕"),
      );
    await until(() => endScene());
    expect(endScene()!.querySelector(".menu-note")?.textContent).toBe(
      "整理前情開新幕；已送出的行動退回草稿",
    );
    act(() => endScene()!.click());
    expect(A.socket().sent.find((f) => f.t === "scene.advance")?.p).toEqual({ round });
    await until(() => B.text().includes("換幕中，GM 正在整理前情…"));
    expect(B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe("我想走");
    const { jobId } = room.frames("x1", "job.scene_advance")[0].p;
    await hub.exec("job.result", { jobId, ok: true, result: { summary: "前情：大家進了酒館。" } });
    await until(() => B.text().includes("第 2 幕 · 第 3 回合"));
    expect(B.q('[data-testid="story"]')!.textContent).toContain("前情摘要前情：大家進了酒館。");

    A.tab("host");
    const sockets = hub.sockets.length;
    // 到場整理的次要動作收進每人的選單，移除仍先確認角色處置
    expect(A.button("移除…")).toBeNull();
    act(() =>
      A.q<HTMLButtonElement>(
        `[data-testid="host-${room.s.members[1].id}"] [aria-haspopup="menu"]`,
      )!.click(),
    );
    expect(A.q('[role="menu"]')!.closest(".app")).not.toBeNull();
    expect([...A.q('[role="menu"]')!.querySelectorAll("button")].at(-1)!.textContent).toBe("封鎖");
    A.click("移除…");
    act(() =>
      A.q<HTMLInputElement>(
        '[data-testid="remove-confirm"] input[type="radio"]:not(:checked)',
      )!.click(),
    ); // 交給 GM 當 NPC
    act(() =>
      [...A.q('[data-testid="remove-confirm"]')!.querySelectorAll("button")]
        .find((x) => x.textContent === "移除")!
        .click(),
    );
    await until(() => room.events("seat.departure").length === 1);
    expect(room.events("seat.departure")[0].payload).toMatchObject({ mode: "npc" });
    await until(() => B.q('[data-testid="fatal"]'));
    expect(B.q('[data-testid="fatal"]')!.textContent).toBe("你已被移出房間");
    await until(() => true, 3);
    expect(hub.sockets.length).toBe(sockets); // 被移出不自動重連

    // 結束房間只在桌面版桌卡（執行端憑證 HTTP）
    await act(async () => {
      await room.run({ kind: "http", route: "close", executorToken: room.executorToken });
      await hub.flush();
    });
    await until(() => A.text().includes("房間已結束"));
    expect(A.q('[data-testid="story"]') ?? A.q('[data-testid="host-panel"]')).toBeTruthy(); // 留著畫面，房主還能匯出
    A.tab("story");
    expect(A.q('[data-testid="action-box"]')).toBeNull(); // 關了就不再給行動格
  });

  it("非房主的回合列沒有換幕", async () => {
    const { A, B } = await table();
    expect(A.button("換幕 ▾")).toBeTruthy();
    expect(B.button("換幕 ▾")).toBeNull();
    expect(B.button("換幕")).toBeNull();
  });

  it("GM 生成中房主沒有換幕", async () => {
    const { room, hub, A } = await table();
    A.click("結算這回合");
    await until(() => room.s.round?.phase === "queued");
    await hub.exec("dispatch.ack", { dispatchId: room.dispatchId() });
    await until(() => room.s.round?.phase === "generating" && A.text().includes("GM 生成中…"));
    expect(A.button("換幕 ▾")).toBeNull();
    expect(A.button("換幕")).toBeNull();
  });

  it("這一幕滿了：不是出額度者的房主點回合列換幕主鈕", async () => {
    const { room, hub, A, B, b } = await table();
    await room.send(A.socket().id, "host.transfer", { memberId: b });
    hub.flush();
    await until(() => B.button("結算這回合"));
    B.click("結算這回合");
    await until(() => room.s.round?.dispatchId && room.s.round.phase === "queued");
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    await hub.exec("gen.failed", {
      dispatchId,
      atPart: null,
      error: {
        kind: "scene_full",
        code: "scene_capacity_full",
        message: "",
        actions: ["scene_advance"],
        resetsAt: null,
      },
    });
    await until(() => B.text().includes("這一幕滿了，請房主換幕"));
    const btn = B.button("換幕") as HTMLButtonElement | null;
    expect(btn?.classList.contains("primary")).toBe(true);
    expect(A.button("換幕")).toBeNull();
    expect(A.button("換幕 ▾")).toBeNull();
    expect(B.button("換幕 ▾")).toBeNull();
    const round = room.s.round!.round;
    B.click("換幕");
    await until(() => B.socket().sent.some((f) => f.t === "scene.advance"));
    expect(B.socket().sent.find((f) => f.t === "scene.advance")?.p).toEqual({ round });
  });

  it("GM 被擋時全員看到公開說明；出額度者看到細節並從面板重試", async () => {
    const { room, hub, A, B } = await table();
    A.click("結算這回合");
    await until(() => room.s.round?.dispatchId && room.s.round.phase === "queued");
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    const error = {
      kind: "refusal",
      code: "AI_CONTENT_FILTERED",
      message: "供應商拒答，建議切境外路線",
      actions: ["switch_route", "switch_model", "retry", "continue"],
      resetsAt: null,
    };
    await hub.exec("gen.failed", { dispatchId, atPart: null, error });
    await until(() => B.text().includes("GM 這段被供應商擋下，等房主處理"));
    expect(B.text()).not.toContain("AI_CONTENT_FILTERED");
    expect(B.text()).toContain("GM 卡住了，房主處理後再送");
    expect(A.text()).toContain("GM 這段被供應商擋下"); // A 是出額度者：只講發生什麼，鈕就在旁邊
    expect(A.text()).not.toContain("等房主處理");
    expect(A.button("重試")).toBeTruthy();
    expect(B.button("重試")).toBeNull();
    expect(B.q('[data-testid="error-detail"]')).toBeNull(); // 細節在回合列，只給出額度者

    await until(() => A.q('[data-testid="error-detail"]'));
    expect(A.q('[data-testid="error-detail"]')!.textContent).toContain(
      "供應商拒答，建議切境外路線",
    );
    // 重新整理（同一台裝置、整份快照）後錯誤細節還在：伺服器在快照帶給出額度者
    const reloaded = phone(hub, `k=${room.joinSecret}`, { storage: A.env.storage });
    await until(() => reloaded.q('[data-testid="tab-payer"]'));
    expect(reloaded.q('[data-testid="error-detail"]')?.textContent).toContain(
      "供應商拒答，建議切境外路線",
    );
    reloaded.tab("payer");
    reloaded.click("重試");
    await until(() => room.frames("x1", "ctl").length === 1);
    const { ctlId, cmd } = room.frames("x1", "ctl")[0].p;
    expect(cmd).toEqual({ kind: "retry" });
    await hub.exec("ctl.result", { ctlId, ok: true });
    await until(() => reloaded.text().includes("GM 生成中…"));
    expect(room.s.round!.phase).toBe("generating");
    expect(reloaded.button("重試")).toBeNull();
  });

  it("卡住時出額度者在回合列換模型：只出伺服器允許的鈕、被拒有錯誤、成功後離開 held", async () => {
    const { room, hub, A, B } = await table();
    A.click("結算這回合");
    await until(() => room.s.round?.dispatchId && room.s.round.phase === "queued");
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    await hub.exec("gen.failed", {
      dispatchId,
      atPart: null,
      error: {
        kind: "model_unavailable",
        code: "AI_HTTP_STATUS_404",
        message: "x",
        actions: ["switch_model", "continue"],
        resetsAt: null,
      },
    });
    await until(() => A.q('[data-testid="held-switch"]'));
    expect(A.button("跳過這段，繼續")).toBeTruthy(); // 伺服器允許的其他動作照留
    expect(B.q('[data-testid="held-switch"]')).toBeNull();
    A.click("換模型 ▾");
    expect(A.q<HTMLSelectElement>('.held-switch select[aria-label="模型"]')!.value).toBe("m-b"); // 下架的原模型灰掉、預設換能用的
    A.click("切換並重送");
    await until(() => room.frames("x1", "ctl").length === 1);
    const first = room.frames("x1", "ctl")[0].p;
    expect(first.cmd).toEqual({ kind: "switch_model", model: "m-b" });
    await hub.exec("ctl.result", {
      ctlId: first.ctlId,
      ok: false,
      error: { code: "AI_HTTP_STATUS_404", message: "還是叫不到" },
    });
    await until(() => A.q('[data-testid="held-error"]'));
    expect(room.s.round!.phase).toBe("held");
    A.click("切換並重送"); // 失敗時彈層留著，可以直接再試
    await until(() => room.frames("x1", "ctl").length === 2);
    await hub.exec("ctl.result", { ctlId: room.frames("x1", "ctl")[1].p.ctlId, ok: true });
    await until(() => room.s.round!.phase === "generating" && A.text().includes("GM 生成中…"));
    expect(A.q('[data-testid="held-switch"]')).toBeNull();
  });

  it("卡住時換模型照伺服器允許的動作送命令：只允許換路線時同路線也送 switch_route；只允許換模型時只列同一條路線", async () => {
    const pick = (P: Phone, model: string) =>
      act(() => {
        const sel = P.q<HTMLSelectElement>('.held-switch select[aria-label="模型"]')!;
        Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(
          sel,
          model,
        );
        sel.dispatchEvent(new Event("change", { bubbles: true }));
      });
    const hold = async (actions: string[]) => {
      const t = await table();
      t.A.click("結算這回合");
      await until(() => t.room.s.round?.dispatchId && t.room.s.round.phase === "queued");
      const dispatchId = t.room.dispatchId();
      await t.hub.exec("dispatch.ack", { dispatchId });
      await t.hub.exec("gen.failed", {
        dispatchId,
        atPart: null,
        error: {
          kind: "provider_unreachable",
          code: "E_GEN_TIMEOUT",
          message: "x",
          actions,
          resetsAt: null,
        },
      });
      await until(() => t.A.q('[data-testid="held-switch"]'));
      t.A.click("換模型 ▾");
      return t;
    };
    const r1 = await hold(["retry", "switch_route"]);
    const route = r1.room.s.sceneSettings!.route;
    pick(r1.A, "m-b");
    r1.A.click("切換並重送");
    await until(() => r1.room.frames("x1", "ctl").length === 1);
    expect(r1.room.frames("x1", "ctl")[0].p.cmd).toEqual({
      kind: "switch_route",
      route,
      model: "m-b",
    });

    const r2 = await hold(["switch_model"]);
    const routes = [
      ...r2.A.el.querySelectorAll<HTMLOptionElement>(
        '.held-switch select[aria-label="路線"] option',
      ),
    ].map((o) => o.value);
    expect(routes).toEqual([r2.room.s.sceneSettings!.route]);
    pick(r2.A, "m-b");
    r2.A.click("切換並重送");
    await until(() => r2.room.frames("x1", "ctl").length === 1);
    expect(r2.room.frames("x1", "ctl")[0].p.cmd).toEqual({ kind: "switch_model", model: "m-b" });
  });

  it("送出後節流中的草稿不會晚到：伺服器草稿保持清空，下一回合不會把舊行動當草稿", async () => {
    const { room, A, a } = await table();
    A.type('[data-testid="action-text"]', "我");
    await until(() => room.s.drafts[a] === "我");
    A.type('[data-testid="action-text"]', "我拔劍"); // 600 ms 內第二則：排在計時器裡
    A.click("送出");
    await until(() => room.events("action.submitted").length === 1);
    await act(() => new Promise((r) => setTimeout(r, 700)));
    expect(room.s.drafts[a]).toBeUndefined();
    expect(
      A.socket()
        .sent.filter((f) => f.t === "action.draft")
        .map((f) => f.p.text),
    ).toEqual(["我"]);
  });

  it("hello 那一刻的錯誤：過期的配對碼照樣加入並提示，可手動輸入配對碼；暱稱不合規停下顯示原因、不重連", async () => {
    const room = await Room.create();
    await room.exec();
    await room.send("x1", "roster.update", { cards: [...CARDS, STAGE] });
    const hub = new Hub(room);
    hub.wrap = (fn) => act(fn);
    const A = phone(hub, `k=${room.joinSecret}&p=ZZZZ-ZZZZ`);
    await joinAndSit(A, "阿明", "角色1");
    await until(() => A.text().includes("配對碼不對或已過期，請在電腦上重發"));
    expect(room.s.payerId).toBeNull();
    A.tab("members");
    A.type('[data-testid="pair-code"]', await room.pairCode(true));
    await act(async () => A.q<HTMLFormElement>(".pair-form")!.requestSubmit());
    await until(() => room.s.payerId === room.s.members[0].id && A.q('[data-testid="tab-payer"]'));

    const B = phone(hub, `k=${room.joinSecret}`);
    B.type('[data-testid="nickname"]', "小\u202e華"); // 雙向覆寫字元：伺服器回 E_NAME_INVALID
    await act(async () => B.q<HTMLFormElement>("form")!.requestSubmit());
    await until(() => B.q('[data-testid="fatal"]'));
    expect(B.q('[data-testid="fatal"]')!.textContent).toBe(
      "名字不能空白，最多 24 個字，不能有特殊控制字元",
    );
    const sockets = hub.sockets.length;
    await until(() => true, 5);
    expect(hub.sockets.length).toBe(sockets);
  });

  it("切回前景時偵測到死連線就重連；收不到回覆的請求逾時回報斷線", async () => {
    let now = Date.now();
    const { room, B, b } = await table({ timing: { requestMs: 100, now: () => now } });
    const old = B.socket();
    old.held = true; // 手機熄螢幕：之後什麼都收不到，onclose 也不來
    B.click("交給 GM");
    await act(() => new Promise((r) => setTimeout(r, 150)));
    await until(() => B.q('[data-testid="action-error"]'));
    expect(B.q('[data-testid="action-error"]')!.textContent).toBe("連線中斷，等重新連上再試");
    expect(room.s.round!.roster.find((e) => e.memberId === b)!.status).toBe("skipped"); // 伺服器其實收到了

    now += 41_000;
    act(() => void document.dispatchEvent(new Event("visibilitychange")));
    await until(() => B.socket() !== old && B.socket()?.readyState === 1 && B.button("收回修改"));
    expect(B.socket().sent[0].p.lastSeq).toBeGreaterThan(0); // 補事件路徑
  });

  it("房主 GM 鈕：只有房主看得到；按旁白時別人已送出的行動留著、GM 說完放回並提示；房主催促只給本人；GM 點到的 NPC 只在房主那邊出現「讓 NPC X 接話」", async () => {
    const { room, hub, A, B } = await table();
    expect(A.q('[data-testid="gm-controls"]')).toBeTruthy();
    expect(B.q('[data-testid="gm-controls"]')).toBeNull();
    // 催一下在狀態列的選單裡，回合列收著也點得到
    act(() => A.q('[data-testid="seat-m2"]')!.closest("button")!.click());
    await until(() => A.q('[data-testid="nudge-m2"]'));
    // 被催的人輸入格收著：催一下就自動展開一次，橫條與輸入框露出、輸入框取得焦點
    act(() => B.q<HTMLButtonElement>('[data-testid="action-tuck"]')!.click());
    await until(() => B.q('[data-testid="action-untuck"]'));
    A.click("催一下");
    await until(() => B.q('[data-testid="nudged-note"]'));
    act(() => A.q('[data-testid="seat-m2"]')!.closest("button")!.click());
    await until(() => A.button("已提醒"));
    expect(B.q('[data-testid="action-untuck"]')).toBeNull();
    await until(() => document.activeElement === B.q('[data-testid="action-text"]'));
    expect(A.q('[data-testid="nudged-note"]')).toBeNull();
    B.type('[data-testid="action-text"]', "還在想");
    B.click("交給 GM");
    await until(() => B.text().includes("這回合交給 GM 安排（結算前可以收回）"));
    expect(B.q('[data-testid="nudged-note"]')).toBeNull();
    const box = () => B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!;
    expect(box().readOnly).toBe(true); // 交給 GM 期間唯讀，草稿留在框裡
    expect(box().value).toBe("還在想");
    await until(() => A.q('[data-testid="gm-keep-note"]'));
    expect(A.q('[data-testid="gm-keep-note"]')!.textContent).toBe(
      "已送出的 1 個行動會保留，GM 說完照常結算",
    );
    expect(B.q('[data-testid="gm-keep-note"]')).toBeNull();
    B.click("收回修改");
    await until(() => B.button("送出") && !box().readOnly);
    expect(box().value).toBe("還在想");
    B.type('[data-testid="action-text"]', "我點酒");
    B.click("送出");
    await until(() => A.q('[data-testid="submitted"]'));
    expect(B.q('[data-testid="nudged-note"]')).toBeNull();
    A.click("GM 旁白");
    await until(() => room.s.round?.kind === "host" && A.q('[data-testid="submitted"]'));
    expect(B.q('[data-testid="submitted-note"]')!.textContent).toContain("我點酒");
    expect(B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe("");
    expect(A.q<HTMLButtonElement>('[data-testid="gm-main"]')!.disabled).toBe(true);
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    await hub.exec("gen.done", { dispatchId, aborted: false, called: "c3" });
    await until(() => A.button("讓 NPC 角色3接話") && B.q('[data-testid="kept-note"]'));
    expect(B.button("收回修改")).toBeTruthy();
    expect(B.button("讓 NPC 角色3接話")).toBeNull();
    A.click("讓 NPC 角色3接話");
    await until(() => room.s.round?.host?.call === true);
  });

  it("GM 分段鈕：選單點了直接執行並成為主鈕；主鈕與選單停用條件一致", async () => {
    const { room, A } = await table();
    act(() => A.q<HTMLButtonElement>(".gm-seg-more")!.click());
    const item = [...A.el.querySelectorAll<HTMLButtonElement>(".menu .menu-item")].find((b) =>
      b.textContent?.startsWith("GM 推進"),
    )!;
    act(() => item.click());
    await until(() => room.s.round?.kind === "host");
    expect(A.q('[data-testid="gm-main"]')!.textContent).toBe("GM 推進");
    expect(A.q<HTMLButtonElement>('[data-testid="gm-main"]')!.disabled).toBe(true);
    expect(A.q<HTMLButtonElement>(".gm-seg-more")!.disabled).toBe(true);
  });

  it("房主收回上一回合：確認後 GM 段從大家的故事串消失、留一行提示；被收回的行動只回到本人，帶回填進輸入框，不用重連", async () => {
    const { room, hub, A, B } = await table();
    B.type('[data-testid="action-text"]', "我推門");
    B.click("送出");
    A.click("交給 GM");
    await until(() => room.s.round?.phase === "queued");
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    await hub.exec("gen.part.begin", { dispatchId, part: 0, speaker: { kind: "gm" } });
    await hub.exec("gen.part.end", {
      dispatchId,
      part: 0,
      text: "門後是一片黑。",
      interrupted: false,
    });
    await hub.exec("gen.done", { dispatchId, aborted: false });
    await until(() => A.button("收回上一回合") && B.text().includes("門後是一片黑。"));
    expect(B.q('[data-testid="undo-controls"]')).toBeNull();
    A.click("收回上一回合");
    await until(() => A.q('[data-testid="undo-confirm"]'));
    A.click("收回上一回合");
    await until(() => room.s.undo);
    await hub.exec("job.result", { jobId: room.s.undo!.jobId, ok: true });
    await until(
      () => !B.text().includes("門後是一片黑。") && B.text().includes("房主收回了上一回合"),
    );
    expect(B.q('[data-testid="story"]')!.textContent).not.toContain("我推門");
    // 輸入框空著：原文直接放回去、上方一行提示，送出後提示消失；伺服器那份另存清掉
    await until(
      () =>
        B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value === "我推門" &&
        B.q('[data-testid="returned-notice"]'),
    );
    expect(B.q('[data-testid="returned"]')).toBeNull();
    await until(() => room.s.returned?.[room.s.members[1].id] === undefined);
    B.click("送出");
    await until(() => !B.q('[data-testid="returned-notice"]'));
  });

  it("收回時已經打了新草稿：草稿不動，原文另外顯示，帶回接在草稿後面", async () => {
    const { room, hub, A, B } = await table();
    B.type('[data-testid="action-text"]', "我推門");
    B.click("送出");
    A.click("交給 GM");
    await until(() => room.s.round?.phase === "queued");
    await room.finishGen("門後是一片黑。");
    hub.flush();
    await until(() => A.button("收回上一回合") && B.q('[data-testid="action-text"]'));
    B.type('[data-testid="action-text"]', "我想先看看");
    A.click("收回上一回合");
    A.click("收回上一回合");
    await until(() => room.s.undo);
    await hub.exec("job.result", { jobId: room.s.undo!.jobId, ok: true });
    await until(() => B.q('[data-testid="returned"]')?.textContent?.includes("我推門"));
    expect(B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe("我想先看看");
    // 合併後太長：不截字、不換掉輸入框，原文照留、提示先刪一些
    const long = "長".repeat(1998);
    B.type('[data-testid="action-text"]', long);
    B.click("帶回");
    await until(() => B.q('[data-testid="returned-too-long"]'));
    expect(B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe(long);
    expect(room.s.returned?.[room.s.members[1].id]).toBe("我推門");
    // 帶回失敗：取消掉的草稿照原樣補送（測試時鐘不走，手動推過草稿限流）
    await until(() => ((room.clock += 600), room.s.drafts[room.s.members[1].id] === long), 2000);
    B.type('[data-testid="action-text"]', "我想先看看");
    B.click("帶回");
    await until(() => !B.q('[data-testid="returned"]'));
    expect(B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe(
      "我想先看看\n我推門",
    );
    expect(room.s.drafts[room.s.members[1].id]).toBe("我想先看看\n我推門");
  });

  it("GM 旁白在第一段前就失敗（held、故事串沒有這回合的段落）：房主照樣看得到收回鈕並能收回", async () => {
    const { room, hub, A } = await table();
    A.click("GM 旁白");
    await until(() => room.s.round?.kind === "host");
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    await hub.exec("gen.failed", {
      dispatchId,
      atPart: 0,
      error: {
        kind: "model_unavailable",
        code: "AI_HTTP_STATUS_404",
        message: "x",
        actions: ["switch_model", "switch_route", "retry"],
        resetsAt: null,
      },
    });
    await until(() => A.button("收回上一回合"));
    // 上次是模型下架：重生的換模型選單灰掉它、預設換成能用的那個
    A.click("重生上一回合");
    await until(() => A.q('[data-testid="undo-confirm"]'));
    const model = A.q<HTMLSelectElement>('[data-testid="undo-confirm"] select[aria-label="模型"]')!;
    expect(model.value).toBe("m-b");
    expect(model.querySelector<HTMLOptionElement>('option[value="m-a"]')!.disabled).toBe(true);
    expect(A.text()).toContain("GM 的模型已下架或叫不到");
    expect(A.text()).toContain("會用選的模型重跑這一回合。"); // 一段都沒寫出來：警語不說「會消失」
    expect(A.button("用原模型重生（已下架）")?.disabled).toBe(true);
    A.click("取消");
    A.click("收回上一回合");
    A.click("收回上一回合");
    await until(() => room.s.undo?.round === 3);
  });
});

describe("玩家卡（room-player-cards）", () => {
  /** env.fetch 接到真的上傳收件（兩殼共用的 receiveUpload），處理完把訊框交給各手機 */
  const uploadFetch = (hub: Hub, calls: string[]) =>
    (async (url: string, init: RequestInit) => {
      calls.push(url);
      const bytes = new Uint8Array(await (init.body as Blob).arrayBuffer());
      const auth = (init.headers as Record<string, string>).Authorization;
      const r = await receiveUpload(hub.room.runner, {
        kind: new URL(url, "https://party.example").searchParams.get("kind"),
        contentLength: String(bytes.length),
        origin: null,
        selfOrigin: "https://party.example",
        deviceId: auth.slice("Bearer ".length),
        body: (async function* () {
          yield bytes;
        })(),
      });
      hub.flush();
      return new Response(JSON.stringify(r.body), { status: r.status });
    }) as typeof fetch;
  const pickFile = (p: Phone, sel: string, file: File) =>
    act(() => {
      const input = p.q<HTMLInputElement>(sel)!;
      Object.defineProperty(input, "files", { value: [file], configurable: true });
      input.dispatchEvent(new Event("change", { bubbles: true }));
    });
  const last = (room: Room, t: string) => room.frames("x1", t).at(-1)!.p;
  const NEW = {
    cardId: "c9",
    name: "匯入的",
    color: null,
    avatar: null,
    blurb: "",
    playable: true,
    hidden: false,
  };

  it("匯入角色卡：太大的檔不送；解析失敗只顯示原因；成功後自動入座", async () => {
    const calls: string[] = [];
    const room = await Room.create();
    await room.exec();
    await room.send("x1", "roster.update", { cards: CARDS });
    const hub = new Hub(room);
    hub.wrap = (fn) => act(fn);
    const C = phone(hub, `k=${room.joinSecret}`, { fetch: uploadFetch(hub, calls) });
    C.type('[data-testid="nickname"]', "小美");
    await act(async () => C.q<HTMLFormElement>("form")!.requestSubmit());
    await until(() => C.q('[data-testid="seat-create"] input[type="file"]'));
    const sel = '[data-testid="seat-create"] input[type="file"]';
    await pickFile(C, sel, new File([new Uint8Array(8 * 1024 * 1024 + 1)], "big.png"));
    await until(() => C.text().includes("檔案太大"));
    expect(calls).toEqual([]);
    await pickFile(C, sel, new File(['{"name":"x"}'], "bad.json"));
    await until(() => room.frames("x1", "job.card_import").length === 1);
    expect(calls[0]).toBe(`/api/rooms/${room.code}/cards?kind=card`);
    await hub.exec("job.result", {
      jobId: last(room, "job.card_import").jobId,
      ok: false,
      error: { code: "E_CARD_INVALID" },
    });
    await until(() => C.text().includes("讀不出這張角色卡"));
    await pickFile(C, sel, new File(['{"name":"匯入的"}'], "ok.json"));
    await until(() => room.frames("x1", "job.card_import").length === 2);
    await hub.exec("roster.update", { cards: [...CARDS, NEW] });
    await hub.exec("job.result", {
      jobId: last(room, "job.card_import").jobId,
      ok: true,
      result: { cardId: "c9" },
    });
    await until(() => C.q('[data-testid="action-box"]'));
    await until(
      () => C.q('[data-testid="import-note"]')?.textContent === "匯入完成：只帶入名字、描述和圖",
    );
  });

  it("建立角色：秘密段不經 seat.create，入座後接著存；存失敗畫面不關、原文保留可重試", async () => {
    const room = await Room.create();
    await room.exec();
    const hub = new Hub(room);
    hub.wrap = (fn) => act(fn);
    const C = phone(hub, `k=${room.joinSecret}`);
    C.type('[data-testid="nickname"]', "小美");
    await act(async () => C.q<HTMLFormElement>("form")!.requestSubmit());
    await until(() => C.button("建立角色"));
    C.click("建立角色");
    C.type('[data-testid="card-public"]', "紅髮劍士");
    C.type('[data-testid="card-private"]', "其實是公主");
    await act(async () => C.q<HTMLFormElement>('[data-testid="card-sheet"]')!.requestSubmit());
    await until(() => room.s.jobs.length > 0);
    expect(JSON.stringify(room.s.jobs)).not.toContain("其實是公主");
    expect(room.s.jobs[0].job).toMatchObject({ name: "小美", blurb: "紅髮劍士" });
    await hub.exec("roster.update", { cards: [{ ...NEW, name: "小美" }] });
    await hub.exec("job.result", {
      jobId: room.s.jobs[0].job.jobId,
      ok: true,
      result: { cardId: "c9" },
    });
    await until(() => room.frames("x1", "card.save").length === 1);
    expect(last(room, "card.save")).toMatchObject({ cardId: "c9", privateMd: "其實是公主" });
    expect(last(room, "card.save").publicMd).toBeUndefined();
    await hub.exec("card.save.result", {
      reqId: last(room, "card.save").reqId,
      ok: false,
      code: "E_INTERNAL",
    });
    await until(() => C.q('[data-testid="card-pending"]'));
    expect(C.q<HTMLTextAreaElement>('[data-testid="card-private"]')!.value).toBe("其實是公主");
    room.clock += 3000; // 存卡每人 2 秒一次
    C.click("重試存檔");
    await until(() => room.frames("x1", "card.save").length === 2);
    await hub.exec("card.save.result", { reqId: last(room, "card.save").reqId, ok: true });
    await until(() => !C.q('[data-testid="card-sheet"]') && C.q('[data-testid="action-box"]'));
    expect(room.store.events.some((e) => JSON.stringify(e).includes("其實是公主"))).toBe(false);
  });

  it("編輯我的角色：讀卡、只送改過的欄位、改名先確認；生圖同一房間只確認一次、預覽標新圖、用這張後標已選用、有未存修改返回先兩選確認、捨棄才撤回、丟掉、按存檔才寫入圖；存檔期間停用欄位；主機關掉生圖只留說明、沒路線整段不出現；別人看不到秘密段", async () => {
    const { room, hub, A, B } = await table();
    const confirm = vi.fn(() => true);
    vi.stubGlobal("confirm", confirm);
    await hub.exec("status", { keepAwake: null, busy: false, imageGen: true });
    A.tab("members"); // 手機：編輯我的角色在名單頁自己那一列
    A.click("編輯我的角色");
    await until(() => room.frames("x1", "card.read").length === 1);
    const card = {
      name: "角色1",
      avatar: "🗡",
      publicMd: "公開",
      privateMd: "舊秘密",
      image: null,
    };
    await hub.exec("card.read.result", { reqId: last(room, "card.read").reqId, ok: true, card });
    await until(() => A.q<HTMLTextAreaElement>('[data-testid="card-private"]')?.value === "舊秘密");
    A.type('[data-testid="card-name"]', "新名字");
    A.type('[data-testid="card-private"]', "新秘密");
    await act(async () => A.q<HTMLFormElement>('[data-testid="card-sheet"]')!.requestSubmit());
    await until(() => room.frames("x1", "card.save").length === 1);
    // 存檔等回覆期間整區停用：存好就關畫面，補打的字會不見
    expect(A.q<HTMLFieldSetElement>('[data-testid="card-fields"]')!.disabled).toBe(true);
    expect(confirm).toHaveBeenCalledWith("要把角色改名成「新名字」嗎？");
    const { reqId, memberId: _m, ...sent } = last(room, "card.save");
    expect(sent).toEqual({ cardId: "c1", name: "新名字", privateMd: "新秘密" });
    await hub.exec("card.save.result", { reqId, ok: true });
    await until(() => !A.q('[data-testid="card-sheet"]'));
    expect(JSON.stringify(room.out[B.socket().id])).not.toMatch(/舊秘密|新秘密/);

    const reopen = async (n: number, c: typeof card) => {
      room.clock += 3000; // 讀卡每人每秒一次、存卡每人 2 秒一次
      A.click("編輯我的角色");
      await until(() => room.frames("x1", "card.read").length === n);
      await hub.exec("card.read.result", {
        reqId: last(room, "card.read").reqId,
        ok: true,
        card: c,
      });
      await until(() => A.q('[data-testid="card-gen-notice"]'));
    };
    const gen = async (n: number) => {
      A.click("AI 生圖");
      await until(() => room.frames("x1", "image.gen").length === n);
      await hub.exec("image.gen.result", {
        reqId: last(room, "image.gen").reqId,
        ok: true,
        file: "1700.png",
        preview: "data:image/png;base64,AAAA",
      });
      await until(() => A.q('[data-testid="card-gen-preview"]'));
    };
    // 「用這張」只進草稿：按返回就撤回，什麼都沒存
    await reopen(2, { ...card, name: "新名字" });
    expect(A.q('[data-testid="card-gen-notice"]')!.textContent).toBe(
      "會用 GM 主機「家裡的電腦」上設定的 AI 額度",
    );
    confirm.mockClear();
    await gen(1);
    expect(confirm).toHaveBeenCalledWith(
      "這會用 GM 主機「家裡的電腦」的 AI 額度生一張圖，要繼續嗎？",
    );
    expect(last(room, "image.gen")).toMatchObject({
      cardId: "c1",
      name: "新名字",
      description: "公開",
      framing: "half",
    });
    expect(A.q('[data-testid="card-gen-preview"]')!.textContent).toContain("新生成的圖");
    // 預覽還沒決定前不能再生，免得以為沒成功又花一次額度
    expect((A.button("AI 生圖") as HTMLButtonElement).disabled).toBe(true);
    // 用這張先過裁切窗，套用才進草稿
    A.click("用這張");
    await until(() => A.q('[data-testid="crop-sheet"]'));
    A.click("套用");
    await until(() =>
      A.q<HTMLImageElement>('[data-testid="card-image-now"]')?.src.startsWith("data:image/png"),
    );
    expect(A.q('[data-testid="card-image-chosen"]')!.textContent).toBe("已選用，按存檔才會套用");
    expect(A.q('[data-testid="card-gen-preview"]')).toBeNull();
    // 有未存的修改按返回：照桌面版兩選確認，焦點預設在「繼續編輯」
    A.click("返回");
    await until(() => A.q('[data-testid="leave-confirm"]'));
    expect(A.q('[data-testid="leave-confirm"]')!.textContent).toContain("有 1 項修改未儲存");
    expect(document.activeElement?.textContent).toBe("繼續編輯");
    expect(A.button("捨棄修改")!.className).toBe("danger");
    A.click("繼續編輯");
    await until(() => !A.q('[data-testid="leave-confirm"]'));
    expect(A.q('[data-testid="card-sheet"]')).not.toBeNull();
    A.click("返回");
    await until(() => A.q('[data-testid="leave-confirm"]'));
    A.click("捨棄修改");
    await until(() => !A.q('[data-testid="card-sheet"]'));
    await reopen(3, { ...card, name: "新名字" });
    expect(A.q('[data-testid="card-image-now"]')).toBeNull();
    expect(room.frames("x1", "card.save")).toHaveLength(1);
    // 同一房間關掉再開不再問；「丟掉」不進草稿；按存檔才把圖庫那張套上
    await gen(2);
    expect(confirm).toHaveBeenCalledTimes(1);
    A.click("丟掉");
    await until(() => !A.q('[data-testid="card-gen-preview"]'));
    expect(A.q('[data-testid="card-image-now"]')).toBeNull();
    await gen(3);
    A.click("用這張");
    await until(() => A.q('[data-testid="crop-sheet"]'));
    A.click("套用");
    await until(() => A.q('[data-testid="card-image-chosen"]'));
    A.click("AI 生圖");
    await until(() => room.frames("x1", "image.gen").length === 4);
    await hub.exec("image.gen.result", {
      reqId: last(room, "image.gen").reqId,
      ok: false,
      code: "E_GEN_QUOTA",
    });
    await until(() => A.text().includes("GM 主機的 AI 額度用完了"));
    A.click("存檔");
    await until(() => room.frames("x1", "card.save").length === 2);
    const { reqId: r2, memberId: _m2, ...sent2 } = last(room, "card.save");
    expect(sent2).toEqual({
      cardId: "c1",
      imageFromGallery: "1700.png",
      crop: { x: 0, y: 0, w: 1, h: 1 },
    });
    await hub.exec("card.save.result", { reqId: r2, ok: true });
    await until(() => !A.q('[data-testid="card-sheet"]'));

    // 主機關掉開關：只留說明，額外提示與構圖一起收起
    await hub.exec("status", { keepAwake: null, busy: false, imageGen: false });
    await reopen(4, card);
    expect(A.q('[data-testid="card-gen-notice"]')!.textContent).toBe("GM 主機關閉了 AI 生圖");
    expect(A.button("AI 生圖")).toBeNull();
    expect(A.q('[data-testid="card-image"] select')).toBeNull();
    // 主機沒有房間能用的生圖路線：整段不出現
    await hub.exec("status", {
      keepAwake: null,
      busy: false,
      imageGen: false,
      imageGenRoute: false,
    });
    await until(() => !A.q('[data-testid="card-gen-notice"]'));
    // 沒有修改：直接返回，不問
    A.click("返回");
    await until(() => !A.q('[data-testid="card-sheet"]'));
  });

  it("從圖庫選：有圖才出現、按了才撈、標使用中、再載入接著最後一張、點縮圖取大預覽再裁切、取消不動草稿、套用才進草稿、存檔送檔名與裁切框", async () => {
    const { room, hub, A } = await table();
    A.tab("members");
    A.click("編輯我的角色");
    await until(() => room.frames("x1", "card.read").length === 1);
    const card = {
      name: "角色1",
      avatar: "🗡",
      publicMd: "公開",
      privateMd: "",
      image: null,
      gallery: 3,
      galleryFile: "1700000000002.png",
    };
    await hub.exec("card.read.result", { reqId: last(room, "card.read").reqId, ok: true, card });
    await until(() => A.button("從圖庫選（3）"));
    expect(room.frames("x1", "card.gallery")).toHaveLength(0);
    A.click("從圖庫選（3）");
    await until(() => room.frames("x1", "card.gallery").length === 1);
    const thumb = "data:image/png;base64,AAAA";
    const page1 = [
      { file: "1700000000003.png", thumb },
      { file: "1700000000002.png", thumb },
    ];
    await hub.exec("card.gallery.result", {
      reqId: last(room, "card.gallery").reqId,
      ok: true,
      items: page1,
      more: 1,
    });
    await until(() => A.q(".card-gallery-thumb"));
    expect(A.q('[data-testid="card-gallery"]')!.textContent).toContain("使用中");
    A.click("載入更多（剩 1 張）");
    await until(() => room.frames("x1", "card.gallery").length === 2);
    expect(last(room, "card.gallery")).toMatchObject({ before: "1700000000002.png" });
    await hub.exec("card.gallery.result", {
      reqId: last(room, "card.gallery").reqId,
      ok: true,
      items: [{ file: "1700000000001.png", thumb }],
      more: 0,
    });
    await until(() => A.el.querySelectorAll(".card-gallery-thumb").length === 3);
    // 點縮圖：取單張預覽 → 裁切窗；取消就什麼都不變
    const thumbs = () => [...A.el.querySelectorAll<HTMLButtonElement>(".card-gallery-thumb")];
    act(() => thumbs()[2].click());
    await until(() => room.frames("x1", "card.gallery").length === 3);
    expect(last(room, "card.gallery")).toMatchObject({ file: "1700000000001.png" });
    expect(thumbs().every((b) => b.disabled)).toBe(true); // 取單張進行中不重複送
    await hub.exec("card.gallery.result", {
      reqId: last(room, "card.gallery").reqId,
      ok: true,
      items: [{ file: "1700000000001.png", thumb }],
    });
    await until(() => A.q('[data-testid="crop-sheet"]'));
    A.click("取消");
    await until(() => !A.q('[data-testid="crop-sheet"]'));
    expect(A.q('[data-testid="card-image-now"]')).toBeNull();
    act(() => thumbs()[2].click());
    await until(() => room.frames("x1", "card.gallery").length === 4);
    await hub.exec("card.gallery.result", {
      reqId: last(room, "card.gallery").reqId,
      ok: true,
      items: [{ file: "1700000000001.png", thumb }],
    });
    await until(() => A.q('[data-testid="crop-sheet"]'));
    // 套用後草稿還在做（切預覽慢）：裁切窗不關、兩顆鈕停用，編卡的存檔與返回也停用
    let finish = () => {};
    vi.stubGlobal(
      "fetch",
      () => new Promise<Response>((r) => (finish = () => r(new Response(new Blob())))),
    );
    A.click("套用");
    await until(() => A.button("套用")?.disabled);
    expect((A.button("取消") as HTMLButtonElement).disabled).toBe(true);
    expect((A.button("存檔") as HTMLButtonElement).disabled).toBe(true);
    expect((A.button("返回") as HTMLButtonElement).disabled).toBe(true);
    expect(A.q('[data-testid="card-image-chosen"]')).toBeNull();
    await act(async () => finish());
    await until(
      () => !A.q('[data-testid="crop-sheet"]') && A.q('[data-testid="card-image-chosen"]'),
    );
    expect((A.button("存檔") as HTMLButtonElement).disabled).toBe(false);
    expect(thumbs()[2].className).toContain("picked");
    A.click("存檔");
    await until(() => room.frames("x1", "card.save").length === 1);
    const { reqId: _r, memberId: _m, ...sent } = last(room, "card.save");
    expect(sent).toEqual({
      cardId: "c1",
      imageFromGallery: "1700000000001.png",
      crop: { x: 0, y: 0, w: 1, h: 1 },
    });
  });

  it("輸入格可收起：草稿還在、點細列展開並聚焦；新回合只自動展開一次；往回捲時結算鈕第一下就送出", async () => {
    const { room, hub, A, B, b } = await table();
    const away = (p: Phone, up: boolean) => {
      const el = p.q<HTMLElement>('[data-testid="story"]')!;
      Object.defineProperty(el, "scrollHeight", { configurable: true, get: () => 2000 });
      Object.defineProperty(el, "clientHeight", { configurable: true, get: () => 400 });
      el.scrollTop = up ? 0 : 2000;
      act(() => el.dispatchEvent(new Event("scroll")));
    };
    B.type('[data-testid="action-text"]', "我先記下");
    await until(() => room.s.drafts[b] === "我先記下");
    act(() => B.q<HTMLButtonElement>('[data-testid="action-tuck"]')!.click());
    expect(B.q('[data-testid="action-text"]')).toBeNull();
    expect(B.q('[data-testid="action-untuck"]')!.textContent).toContain("✎ 草稿");
    expect(B.q('[data-testid="action-untuck"]')!.textContent).toContain("打字中");
    expect(B.button("交給 GM")).toBeNull();
    expect(room.s.drafts[b]).toBe("我先記下");
    act(() => B.q<HTMLButtonElement>('[data-testid="action-untuck"]')!.click());
    const area = B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!;
    expect(area.value).toBe("我先記下");
    expect(document.activeElement).toBe(area);
    expect(B.button("交給 GM")).toBeTruthy();
    act(() => A.q<HTMLButtonElement>('[data-testid="action-tuck"]')!.click());
    expect(A.button("交給 GM")).toBeNull();
    expect(A.q('[data-testid="gm-controls"]')).toBeTruthy();
    act(() => A.q<HTMLButtonElement>('[data-testid="action-untuck"]')!.click());
    act(() => A.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.blur());

    act(() => B.q<HTMLButtonElement>('[data-testid="action-tuck"]')!.click());
    A.click("結算這回合");
    await until(() => room.s.round?.phase === "queued");
    await room.finishGen("下一段");
    hub.flush();
    await until(() => B.q('[data-testid="action-text"]'));
    act(() => B.q<HTMLButtonElement>('[data-testid="action-tuck"]')!.click());
    A.type('[data-testid="action-text"]', "房主也在打");
    await until(() => room.s.drafts[room.s.members[0].id] === "房主也在打");
    expect(B.q('[data-testid="action-text"]')).toBeNull();

    away(A, true);
    expect(A.q(".room-screen")!.classList.contains("reading")).toBe(true);
    act(() => A.q<HTMLButtonElement>('[data-testid="round-summary"]')!.click());
    // 第一下只結束閱讀、不展開詳情
    expect(A.q('[data-testid="round-summary"]')!.getAttribute("aria-expanded")).toBe("false");
    expect(A.q(".room-screen")!.classList.contains("reading")).toBe(false);
    away(A, false);
    away(A, true);
    expect(A.q(".room-screen")!.classList.contains("reading")).toBe(true);
    act(() => A.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.focus());
    expect(A.q(".room-screen")!.classList.contains("reading")).toBe(false);
    act(() => A.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.blur());
    expect(A.q(".room-screen")!.classList.contains("reading")).toBe(false);
    away(A, true);
    expect(A.q(".room-screen")!.classList.contains("reading")).toBe(true);
    A.click("結算這回合");
    await until(() => room.s.round?.phase === "queued");
  });

  it("往回捲打過字後，失焦再點送出仍送出一次，輸入列還在", async () => {
    const { room, B } = await table();
    const away = (up: boolean) => {
      const el = B.q<HTMLElement>('[data-testid="story"]')!;
      Object.defineProperty(el, "scrollHeight", { configurable: true, get: () => 2000 });
      Object.defineProperty(el, "clientHeight", { configurable: true, get: () => 400 });
      el.scrollTop = up ? 0 : 2000;
      act(() => el.dispatchEvent(new Event("scroll")));
    };
    B.type('[data-testid="action-text"]', "我推門");
    const area = B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!;
    act(() => area.focus());
    away(true);
    act(() => area.blur());
    expect(B.q(".room-screen")!.classList.contains("reading")).toBe(false);
    B.click("送出");
    await until(() => room.events("action.submitted").length === 1);
    expect(room.events("action.submitted")).toHaveLength(1);
    expect(room.events("action.submitted")[0].payload.text).toBe("我推門");
    expect(B.q('[data-testid="action-text"]')).toBeTruthy();
    expect(B.q(".room-screen")!.classList.contains("reading")).toBe(false);
  });
});

describe("往回捲閱讀時被催", () => {
  it("行動格只露出催促橫條，輸入列照舊收起", async () => {
    const css = document.createElement("style");
    // vitest 不載 CSS：直接讀檔塞進頁面，讓 getComputedStyle 套真的閱讀模式規則
    css.textContent = readFileSync(`${process.cwd()}/src/features/room/phone-layout.css`, "utf8");
    document.head.append(css);
    try {
      const { B, A } = await table();
      const el = B.q<HTMLElement>('[data-testid="story"]')!;
      Object.defineProperty(el, "scrollHeight", { configurable: true, get: () => 2000 });
      Object.defineProperty(el, "clientHeight", { configurable: true, get: () => 400 });
      el.scrollTop = 0;
      act(() => el.dispatchEvent(new Event("scroll")));
      expect(B.q(".room-screen")!.classList.contains("reading")).toBe(true);
      const shown = (sel: string) => getComputedStyle(B.q<HTMLElement>(sel)!).display !== "none";
      expect(shown('[data-testid="action-box"]')).toBe(false); // 沒被催：整格收起（原規則）
      expect(shown('[data-testid="seat-strip"]')).toBe(false);
      act(() => A.q('[data-testid="seat-m2"]')!.closest("button")!.click());
      await until(() => A.q('[data-testid="nudge-m2"]'));
      A.click("催一下");
      await until(() => B.q('[data-testid="nudged-note"]'));
      expect(B.q(".room-screen")!.classList.contains("reading")).toBe(true);
      expect(shown('[data-testid="action-box"]')).toBe(true);
      expect(shown('[data-testid="nudged-note"]')).toBe(true);
      expect(shown(".action-row")).toBe(false);
    } finally {
      css.remove();
    }
  });
});

describe("AI 代管（room-seat-delegate 包 3）", () => {
  const inside = (p: Phone, testId: string, label: string) =>
    act(() =>
      [...p.q(`[data-testid="${testId}"]`)!.querySelectorAll("button")]
        .find((x) => x.textContent === label)!
        .click(),
    );

  it("已送出後自己交給 AI 代管：先確認，原文回到輸入框、常駐列出現、送出停用；別人名單看到標籤；接回後收起", async () => {
    const { room, A, B, b } = await table();
    B.type('[data-testid="action-text"]', "我先走");
    B.click("送出");
    await until(() => room.events("action.submitted").length === 1);
    B.tab("members");
    B.click("交給 AI 代管");
    expect(B.q('[data-testid="delegate-confirm"]')!.textContent).toContain(
      "你這回合已送出的行動會退回草稿、不計入",
    );
    inside(B, "delegate-confirm", "交給 AI 代管");
    await until(() => B.q('[data-testid="delegated-banner"]'));
    expect(B.q('[data-testid="delegated-banner"]')!.textContent).toContain(
      "接回後從下一回合開始行動",
    );
    B.tab("story");
    expect(B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe("我先走");
    expect(B.text()).toContain("你的角色代管中，接回後才能送行動");
    expect(B.button("送出")!.disabled).toBe(true);
    A.tab("members");
    await until(() => A.q(`[data-testid="member-${b}"]`)!.textContent!.includes("AI 代管中"));
    B.click("接回");
    await until(() => !B.q('[data-testid="delegated-banner"]'));
    expect(room.s.members.find((m) => m.id === b)!.delegated).toBe(false);
    expect(B.q('[data-testid="delegate-returned"]')!.textContent).toBe(
      "你離開期間的劇情在上面，往上翻可以看",
    );
    B.type('[data-testid="action-text"]', "我回來了");
    expect(B.q('[data-testid="delegate-returned"]')).toBeNull();
  });

  it("暫時離開：遊戲中已入座跳兩鈕確認，「交給 AI 代管再離開」先代管再斷線", async () => {
    const { room, B, b } = await table();
    B.tab("members");
    B.click("暫時離開");
    expect(B.q('[data-testid="leave-confirm"]')!.textContent).toContain(
      "你的座位會保留。要不要先把角色交給 AI 代管？",
    );
    inside(B, "leave-confirm", "交給 AI 代管再離開");
    await until(() => B.text().includes("你已暫時離開"));
    expect(room.s.members.find((m) => m.id === b)!.delegated).toBe(true);
  });

  it("房主從狀態列代管離線的人；全員代管時房主與玩家看到不同的原因，接回寫馬上能行動", async () => {
    const { room, A, B, b } = await table();
    B.tab("members");
    B.click("暫時離開");
    inside(B, "leave-confirm", "直接離開（座位保留）");
    await until(() => A.q(`[data-testid="seat-${b}"]`)!.textContent!.includes("離線"));
    const offline = A.q<HTMLElement>(`[data-testid="seat-${b}"]`)!;
    expect((offline.closest(".seat-slot") as HTMLElement).style.opacity).toBe("0.5");
    act(() => offline.closest("button")!.click());
    A.click("交給 AI 代管");
    await until(() => room.s.members.find((m) => m.id === b)!.delegated);

    const { room: r2, A: A2, B: B2, a: a2, b: b2 } = await table();
    A2.tab("host");
    act(() => A2.q<HTMLButtonElement>(`[data-testid="attend-delegate-${b2}"]`)!.click());
    await until(() => r2.s.members.find((m) => m.id === b2)!.delegated);
    act(() => A2.q<HTMLButtonElement>(`[data-testid="attend-delegate-${a2}"]`)!.click());
    await until(() => A2.text().includes("全員代管中：按 GM 推進，或取消某人的代管"));
    await until(() => B2.text().includes("全員代管中，等房主處理"));
    expect(A2.button("結算這回合")).toBeNull(); // 名單空著：結算鈕不出現（按了也只會被擋）
    expect(B2.q('[data-testid="delegated-banner"]')!.textContent).toContain("接回後馬上就能行動");
  });

  it("到場整理：一鍵代管沒連線的人；邀請新朋友顯示現行連結不重生；暫停房間先確認、送出後停用", async () => {
    const { room, A, B, b } = await table();
    B.tab("members");
    B.click("暫時離開");
    inside(B, "leave-confirm", "直接離開（座位保留）");
    A.tab("host");
    await until(() => A.q('[data-testid="delegate-offline"]'));
    expect(A.q('[data-testid="delegate-offline"]')!.textContent).toBe(
      "把沒連線的人都交給 AI 代管（1 人）",
    );
    act(() => A.q<HTMLButtonElement>('[data-testid="delegate-offline"]')!.click());
    await until(() => !A.q('[data-testid="delegate-offline"]'));
    expect(A.q(`[data-testid="host-${b}"]`)!.textContent).toContain("AI 代管中");

    A.click("邀請新朋友");
    await until(() => A.q('[data-testid="invite"]'));
    expect(A.q<HTMLInputElement>('[data-testid="invite"]')!.value).toBe(
      `https://party.example/r/${room.code}#k=${room.joinSecret}`,
    );
    expect(room.frames("x1", "invite")).toEqual([]);

    A.click("暫停房間");
    expect(A.q('[data-testid="pause-confirm"]')!.textContent).toContain("得在電腦（");
    inside(A, "pause-confirm", "暫停房間");
    await until(() => room.frames("x1", "ctl").length === 1);
    expect(room.frames("x1", "ctl")[0].p.cmd).toEqual({ kind: "pause" });
    expect(A.q<HTMLButtonElement>('[data-testid="pause-room"]')!.disabled).toBe(true);
  });
});

describe("名單與接回提示（room-desktop-layout 包 C）", () => {
  it("讓出座位需確認，取消保留座位，確認才釋放", async () => {
    const { room, A, B, b } = await table();
    A.tab("members");
    expect(A.text()).toContain("用我的額度");
    B.tab("members");
    expect(B.q(".badge[title]")!.getAttribute("title")).toBe("GM 用這個人的 AI 額度在跑");
    expect(B.text()).toContain("用他的額度");
    B.click("讓出座位");
    expect(room.s.members.find((m) => m.id === b)!.cardId).toBe("c2");
    expect(B.q('[data-testid="seat-release-confirm"]')!.textContent).toContain(
      "你將不再控制這個角色",
    );
    B.click("取消");
    expect(B.q('[data-testid="seat-release-confirm"]')).toBeNull();
    // 失敗也收起遮罩，名單頁錯誤可見，座位保留。
    const socket = B.socket();
    const rejected = vi.spyOn(socket, "send").mockImplementationOnce((raw) => {
      const frame = JSON.parse(raw);
      queueMicrotask(() =>
        socket.onmessage?.({
          data: JSON.stringify({ t: "err", p: { id: frame.id, code: "E_FORBIDDEN" } }),
        }),
      );
    });
    B.click("讓出座位");
    act(() => B.q<HTMLButtonElement>('[data-testid="seat-release-confirm"] .danger')!.click());
    await until(() => !B.q('[data-testid="seat-release-confirm"]'));
    expect(B.q('[data-testid="members"] .field-error')!.textContent).not.toBe("");
    expect(room.s.members.find((m) => m.id === b)!.cardId).toBe("c2");
    rejected.mockRestore();
    B.click("讓出座位");
    act(() => B.q<HTMLButtonElement>('[data-testid="seat-release-confirm"] .danger')!.click());
    await until(() => !room.s.members.find((m) => m.id === b)!.cardId);
    expect(B.q('[data-testid="delegate-returned"]')).toBeNull();
    // 沒入座段保留指派座位，次要操作仍在選單內
    A.tab("host");
    const row = A.q(`[data-testid="host-${b}"]`)!;
    expect(row.querySelector("select")).not.toBeNull();
    expect(row.textContent).not.toContain("轉讓房主");
    act(() => row.querySelector<HTMLButtonElement>('[aria-haspopup="menu"]')!.click());
    expect(A.q('[role="menu"]')!.textContent).toContain("轉讓房主");
    expect(A.q('[role="menu"]')!.textContent).toContain("移除");
  });

  it("房主替別人取消代管不出提示；本人從名單接回後提示五秒自動消失", async () => {
    const { room, A, B, b } = await table();
    B.tab("members");
    B.click("交給 AI 代管");
    await until(() => B.q('[data-testid="delegated-banner"]'));
    A.tab("host");
    act(() => A.q<HTMLButtonElement>(`[data-testid="attend-delegate-${b}"]`)!.click());
    await until(() => !B.q('[data-testid="delegated-banner"]'));
    expect(A.q('[data-testid="delegate-returned"]')).toBeNull();
    expect(B.q('[data-testid="delegate-returned"]')).toBeNull();
    B.click("交給 AI 代管");
    await until(() => room.s.members.find((m) => m.id === b)!.delegated);
    act(() => B.q<HTMLButtonElement>('[data-testid="self-delegate"]')!.click());
    await until(() => B.q('[data-testid="delegate-returned"]'));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 5050));
    });
    expect(B.q('[data-testid="delegate-returned"]')).toBeNull();
  }, 10000);
});

describe("桌邊狀態列與房主提醒", () => {
  const chip = (p: Phone, id: string) => p.q<HTMLElement>(`[data-testid="seat-${id}"]`)!;

  it("徽章從打字中到已送出、交給 GM；非房主不可點；房主看到還剩幾人", async () => {
    const { room, A, B, a, b } = await table();
    const ids = [...A.el.querySelectorAll("[data-testid^=seat-m]")].map((el) =>
      el.getAttribute("data-testid"),
    );
    expect(ids).toEqual([`seat-${a}`, `seat-${b}`]);
    expect(chip(A, a).closest("button")).toBeNull();
    expect(chip(B, a).closest("button")).toBeNull();
    expect(chip(A, b).closest("button")).toBeTruthy();
    expect(chip(A, b).closest("button")!.getAttribute("title")).toBe("小華 · 角色2");
    B.type('[data-testid="action-text"]', "我推門");
    await until(() => room.s.drafts[b] === "我推門");
    await until(
      () => chip(A, b).textContent!.includes("打字中") && chip(A, b).querySelector(".seat-dots"),
    );
    B.click("送出");
    await until(() => chip(A, b).textContent!.includes("✓ 已送出"));
    expect(A.text()).toContain("角色2 已送出，還剩 1 人");
    expect(B.q('[data-testid="host-toasts"]')).toBeNull();
    // 點了就收：只收那一條（前面「選了角色」的還在）
    const toasts = () => A.el.querySelectorAll('[data-testid="host-toast"]').length;
    const before = toasts();
    act(() => A.q<HTMLButtonElement>('[data-testid="host-toast"]')!.click());
    expect(toasts()).toBe(before - 1);
    B.click("收回修改");
    await until(() => chip(A, b).textContent!.includes("打字中"));
    B.click("交給 GM");
    await until(() => chip(A, b).textContent!.includes("交給 GM"));
    expect(A.text()).toContain("角色2 交給 GM，還剩 1 人");
    B.click("收回修改");
    await until(() => !chip(A, b).textContent!.includes("交給 GM"));
    act(() => chip(A, b).closest("button")!.click());
    await until(() => A.q(`[data-testid="nudge-${b}"]`));
    A.click("催一下");
    await until(() => B.q('[data-testid="nudged-note"]'));
    expect(A.socket().sent.some((f) => f.t === "host.nudge" && f.p.memberId === b)).toBe(true);
    act(() => chip(A, b).closest("button")!.click());
    expect(A.button("已提醒")!.getAttribute("aria-disabled")).toBe("true");
  });

  it("大廳列出還沒選角的人，房主收到進房提醒", async () => {
    const room = await Room.create();
    await room.exec();
    await room.send("x1", "roster.update", { cards: [...CARDS, STAGE] });
    const hub = new Hub(room);
    hub.wrap = (fn) => act(fn);
    const A = phone(hub, `k=${room.joinSecret}&p=${await room.pairCode(true)}`);
    await joinAndSit(A, "阿明", "角色1");
    const B = phone(hub, `k=${room.joinSecret}`);
    B.type('[data-testid="nickname"]', "小華");
    await act(async () => B.q<HTMLFormElement>("form")!.requestSubmit());
    await until(() => chip(A, "m2").textContent?.includes("還沒選角"));
    expect(chip(A, "m2").textContent).toContain("小華");
    expect((chip(A, "m2").closest(".seat-slot") as HTMLElement).style.opacity).toBe("0.5");
    expect(chip(A, "m2").closest("button")).toBeNull();
    expect(chip(A, "m1").textContent).toContain("角色1");
    expect((chip(A, "m1").closest(".seat-slot") as HTMLElement).style.opacity).toBe("");
    expect(A.text()).toContain("小華 進房了，還沒選角");
    expect(B.q('[data-testid="host-toasts"]')).toBeNull();
  });
});

describe("房主當場開關倒數", () => {
  /** 測試時鐘與畫面時鐘對齊，摘要才會正好是「剩 2:00」 */
  function pinClock(room: Room) {
    room.clock = Date.now();
    return vi.spyOn(Date, "now").mockReturnValue(room.clock);
  }

  async function startTwoMinutes(A: Phone) {
    A.click("⏱ 倒數 ▾");
    const item = () =>
      [...A.el.querySelectorAll<HTMLButtonElement>(".menu .menu-item")].find(
        (b) => b.textContent === "2 分鐘",
      );
    await until(() => item());
    act(() => item()!.click());
  }

  it("房主開 2 分鐘：玩家看到橫條與「剩 2:00」", async () => {
    const { room, A, B } = await table();
    const spy = pinClock(room);
    try {
      await startTwoMinutes(A);
      await until(
        () =>
          B.q('[data-testid="host-timer-note"]')?.textContent === "房主開了倒數：2 分鐘內送出" &&
          B.text().includes("剩 2:00"),
      );
      expect(A.button("停止倒數")).toBeTruthy();
      expect(A.q('[data-testid="host-timer-note"]')).toBeNull(); // 房主自己不看橫條
    } finally {
      spy.mockRestore();
    }
  });

  it("停止倒數：橫條消失，摘要回到「輪到大家行動」", async () => {
    const { room, A, B } = await table();
    const spy = pinClock(room);
    try {
      await startTwoMinutes(A);
      await until(() => B.q('[data-testid="host-timer-note"]'));
      A.click("停止倒數");
      await until(
        () => !B.q('[data-testid="host-timer-note"]') && B.text().includes("輪到大家行動"),
      );
      expect(A.button("⏱ 倒數 ▾")).toBeTruthy();
    } finally {
      spy.mockRestore();
    }
  });

  it("玩家收起輸入格後房主開倒數：輸入格展開、看得到橫條", async () => {
    const { room, A, B } = await table();
    const spy = pinClock(room);
    try {
      act(() => B.q<HTMLButtonElement>('[data-testid="action-tuck"]')!.click());
      await until(() => B.q('[data-testid="action-untuck"]'));
      await startTwoMinutes(A);
      await until(
        () => B.q('[data-testid="host-timer-note"]') && !B.q('[data-testid="action-untuck"]'),
      );
    } finally {
      spy.mockRestore();
    }
  });

  it("非房主沒有倒數鈕", async () => {
    const { A, B } = await table();
    expect(A.button("⏱ 倒數 ▾")).toBeTruthy();
    expect(B.button("⏱ 倒數 ▾")).toBeNull();
    expect(B.button("停止倒數")).toBeNull();
  });
});

describe("桌邊聊天", () => {
  const sendChat = (p: Phone, text: string) => {
    p.tab("chat");
    p.type('[data-testid="chat-text"]', text);
    act(() => p.q<HTMLButtonElement>('[data-testid="chat-send"]')!.click());
  };
  const pickTo = (p: Phone, id: string) =>
    act(() => {
      const sel = p.q<HTMLSelectElement>('[data-testid="chat-to"]')!;
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")!.set!.call(sel, id);
      sel.dispatchEvent(new Event("change", { bubbles: true }));
    });

  it("公開訊息對方看得到、未讀 1，切到聊天頁歸零", async () => {
    const { A, B } = await table();
    sendChat(A, "我先去找酒保");
    await until(() => B.q('[data-testid="chat-unread"]')?.textContent === "1");
    expect(B.q('[data-testid="chat-notice"]')!.textContent).toContain("阿明：我先去找酒保");
    B.tab("chat");
    await until(() => B.q('[data-testid="chat-list"]')!.textContent!.includes("我先去找酒保"));
    expect(B.q('[data-testid="chat-unread"]')).toBeNull();
    expect(B.text()).toContain("房主 → 大家");
  });

  it("私訊第三人看不到；提示列點了切到聊天頁", async () => {
    const { hub, room, A, B } = await table();
    const C = phone(hub, `k=${room.joinSecret}`);
    C.type('[data-testid="nickname"]', "阿凱");
    await act(async () => C.q<HTMLFormElement>("form")!.requestSubmit());
    await until(() => C.q('[data-testid="tab-chat"]'));
    A.tab("chat");
    pickTo(A, room.s.members[1].id);
    sendChat(A, "這句只有小華");
    await until(() => B.q('[data-testid="chat-notice"]')?.textContent?.includes("這句只有小華"));
    expect(C.q('[data-testid="chat-unread"]')).toBeNull();
    expect(C.q('[data-testid="chat-notice"]')).toBeNull();
    act(() => B.q<HTMLButtonElement>('[data-testid="chat-notice"]')!.click());
    await until(() => B.q('[data-testid="chat-list"]')!.textContent!.includes("你（私訊）"));
    expect(B.q('[data-testid="chat-unread"]')).toBeNull();
    C.tab("chat");
    expect(C.q('[data-testid="chat-list"]')!.textContent).not.toContain("這句只有小華");
    expect(A.q<HTMLSelectElement>('[data-testid="chat-to"]')!.value).toBe(room.s.members[1].id);
    expect(A.text()).toContain("你 → 小華（私訊）");
    expect(B.q('[data-testid="chat-text"]')!.getAttribute("placeholder")).toBe("跟大家說…");
    expect(A.q('[data-testid="chat-text"]')!.getAttribute("placeholder")).toBe("私訊 小華…");
    // 切到故事再回來：對象與沒送的字都還在
    A.type('[data-testid="chat-text"]', "還沒送的");
    A.tab("story");
    A.tab("chat");
    expect(A.q<HTMLSelectElement>('[data-testid="chat-to"]')!.value).toBe(room.s.members[1].id);
    expect(A.q<HTMLTextAreaElement>('[data-testid="chat-text"]')!.value).toBe("還沒送的");
  });

  it("送出只清那一次送的內容，等回覆期間接著打的字留著", async () => {
    const { A, B } = await table();
    A.tab("chat");
    A.type('[data-testid="chat-text"]', "第一句");
    act(() => A.q<HTMLButtonElement>('[data-testid="chat-send"]')!.click());
    A.type('[data-testid="chat-text"]', "第一句第二句"); // 回覆還沒回來就繼續打
    await until(() => B.q('[data-testid="chat-notice"]'));
    await until(() => A.q<HTMLTextAreaElement>('[data-testid="chat-text"]')!.value === "第二句");
  });

  it("超過 500 字送出鈕停用並寫剩餘", async () => {
    const { A } = await table();
    A.tab("chat");
    A.type('[data-testid="chat-text"]', "測".repeat(501));
    expect(A.q<HTMLButtonElement>('[data-testid="chat-send"]')!.disabled).toBe(true);
    expect(A.q('[data-testid="chat-over"]')!.textContent).toContain("還剩 -1 字");
    A.type('[data-testid="chat-text"]', "測".repeat(500));
    expect(A.q<HTMLButtonElement>('[data-testid="chat-send"]')!.disabled).toBe(false);
    expect(A.q('[data-testid="chat-over"]')).toBeNull();
  });

  it("重新整理後舊訊息不算未讀", async () => {
    const { hub, room, A, B } = await table();
    sendChat(A, "剛才那句");
    await until(() => B.q('[data-testid="chat-unread"]')?.textContent === "1");
    const again = phone(hub, `k=${room.joinSecret}`, { storage: B.env.storage });
    await until(() => again.q('[data-testid="tab-chat"]'));
    expect(again.q('[data-testid="chat-unread"]')).toBeNull();
    expect(again.q('[data-testid="chat-notice"]')).toBeNull();
    again.tab("chat");
    expect(again.q('[data-testid="chat-list"]')!.textContent).toContain("剛才那句");
  });

  it("故事頁斷線重連期間到的訊息算未讀", async () => {
    let now = Date.now();
    const { room, A, B } = await table({ timing: { now: () => now } });
    const old = B.socket();
    old.held = true;
    sendChat(A, "你斷線時這句到了");
    await until(() => room.s.chat?.length === 1);
    expect(B.q('[data-testid="chat-unread"]')).toBeNull();
    now += 41_000;
    act(() => void document.dispatchEvent(new Event("visibilitychange")));
    await until(
      () => B.socket() !== old && B.q('[data-testid="chat-unread"]')?.textContent === "1",
    );
    expect(B.q('[data-testid="chat-notice"]')!.textContent).toContain("你斷線時這句到了");
  });

  it("聊天頁開著但頁面不可見時收到訊息不歸零，恢復可見才歸零", async () => {
    const { A, B } = await table();
    B.tab("chat");
    let hidden = false;
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => (hidden ? "hidden" : "visible"),
    });
    try {
      hidden = true;
      act(() => void document.dispatchEvent(new Event("visibilitychange")));
      sendChat(A, "螢幕暗著");
      await until(() => B.q('[data-testid="chat-unread"]')?.textContent === "1");
      expect(B.q('[data-testid="chat-list"]')!.textContent).toContain("螢幕暗著");
      hidden = false;
      act(() => void document.dispatchEvent(new Event("visibilitychange")));
      await until(() => !B.q('[data-testid="chat-unread"]'));
    } finally {
      delete (document as { visibilityState?: string }).visibilityState;
    }
  });

  it("手機 Enter 不送出；桌面 Enter 送出，組字中與 Shift+Enter 不送", async () => {
    const { room, A } = await table();
    A.tab("chat");
    A.type('[data-testid="chat-text"]', "換行就好");
    const area = A.q<HTMLTextAreaElement>('[data-testid="chat-text"]')!;
    const enter = new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true });
    act(() => area.dispatchEvent(enter));
    await act(async () => void (await new Promise((r) => setTimeout(r, 30))));
    expect(room.s.chat ?? []).toHaveLength(0);
    expect(enter.defaultPrevented).toBe(false);

    const sent: unknown[] = [];
    const el = document.body.appendChild(document.createElement("div"));
    const root = createRoot(el);
    mounted.push(root);
    const view = {
      chat: [],
      members: [
        { memberId: "m1", nickname: "阿明", cardId: null, connected: true, delegated: false },
        { memberId: "m2", nickname: "小華", cardId: null, connected: true, delegated: false },
      ],
      me: {
        memberId: "m1",
        nickname: "阿明",
        cardId: null,
        isHost: true,
        isPayer: true,
        draft: null,
        returned: null,
        delegated: false,
      },
      room: { hostId: "m1" },
    } as unknown as RoomView;
    const actSend: Act = async (_t, p) => {
      sent.push(p);
      return { ok: true, result: { id: 1 } };
    };
    act(() => root.render(<ChatPanel view={view} act={actSend} desktop hidden={false} />));
    const box = el.querySelector<HTMLTextAreaElement>('[data-testid="chat-text"]')!;
    const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!;
    act(() => {
      set.call(box, "桌面這句");
      box.dispatchEvent(new Event("input", { bubbles: true }));
    });
    const plain = new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true });
    act(() => box.dispatchEvent(plain));
    await until(() => sent.length === 1);
    expect(plain.defaultPrevented).toBe(true);
    expect(sent[0]).toEqual({ text: "桌面這句" });

    act(() => {
      set.call(box, "組字");
      box.dispatchEvent(new Event("input", { bubbles: true }));
    });
    const ime = new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true });
    Object.defineProperty(ime, "isComposing", { value: true });
    Object.defineProperty(ime, "keyCode", { value: 229 });
    act(() => box.dispatchEvent(ime));
    const shift = new KeyboardEvent("keydown", {
      key: "Enter",
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    act(() => box.dispatchEvent(shift));
    await act(async () => void (await new Promise((r) => setTimeout(r, 30))));
    expect(sent).toHaveLength(1);
    expect(ime.defaultPrevented).toBe(false);
    expect(shift.defaultPrevented).toBe(false);
  });

  it("入座名單沒頭像畫角色名首字，有頭像用圖", () => {
    const el = document.body.appendChild(document.createElement("div"));
    const root = createRoot(el);
    mounted.push(root);
    const card = (name: string, avatar: string | null) => ({
      cardId: name,
      name,
      color: "#3366ff",
      avatar,
      blurb: "",
      playable: true,
      hidden: false,
      claimedBy: null,
    });
    const view = {
      cards: [card("莉亞", null), card("老闆", "data:image/jpeg;base64,xx")],
      room: { settings: { allowCustomCards: false } },
    } as unknown as RoomView;
    const actSend: Act = async () => ({ ok: true });
    act(() =>
      root.render(<SeatPicker view={view} act={actSend} upload={null} onCreate={() => {}} />),
    );
    const rows = [...el.querySelectorAll(".seat-card")];
    const letter = rows[0].querySelector(".seat-avatar");
    expect(letter?.tagName).toBe("SPAN");
    expect(letter?.textContent).toBe("莉");
    expect(rows[1].querySelector("img.seat-avatar")?.getAttribute("src")).toBe(
      "data:image/jpeg;base64,xx",
    );
  });

  it("選角清單的簡介剝掉 Markdown：標題開頭的卡顯示內文", () => {
    expect(
      seatBlurb("### 簡介\n**雄狼獸人**傭兵，[北境](https://x.test)出身\n- 擅長*雙斧*", "格羅"),
    ).toBe("雄狼獸人傭兵，北境出身 擅長雙斧");
    expect(seatBlurb("# 格羅\n---\n![圖](a.png)", "格羅")).toBe("格羅");
    expect(seatBlurb("<b>{{char}}</b> 是獅獸人鐵匠", "雷恩")).toBe("雷恩 是獅獸人鐵匠");
    expect(seatBlurb("純文字，2*3 不動", "雷恩")).toBe("純文字，2*3 不動");
    expect(seatBlurb("看[北境](https://x.test/a(b))的地圖", "雷恩")).toBe("看北境的地圖");
    expect(seatBlurb("簡介\n====\n熊獸人酒保", "雷恩")).toBe("熊獸人酒保");
    expect(seatBlurb("雄狼傭兵\n\n---\n\n北境出身", "雷恩")).toBe("雄狼傭兵 北境出身");
    expect(seatBlurb("<style>.a{color:red}</style>熊獸人<!-- 註 -->酒保", "雷恩")).toBe(
      "熊獸人酒保",
    );
    expect(seatBlurb("獅獸人<style>.a{color:red", "雷恩")).toBe("獅獸人");
  });
});
