// @vitest-environment happy-dom
// 手機客戶端整合測試：真的伺服器核心（room-test-kit）當假伺服器，兩支「手機」各自渲染 App，透過假 WebSocket 連進同一房。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { MemoryStorage } from "@web/features/openrouter/memory-storage";
import { CARDS, Room, STAGE } from "../../../server/core/room-test-kit";
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
      useEffect(() => onCropComplete({ x: 0, y: 0, width: 100, height: 100 }, { x: 0, y: 0, width: 1, height: 1 }), []);
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
  const env = { href: `https://party.example/r/${hub.room.code}#${hash}`, storage: new MemoryStorage(), open, fetch, userAgent: "Mozilla/5.0", replaceHref: (h: string) => void hrefs.push(h), retryDelays: [0], ...extra };
  act(() => root.render(<App env={env} lang="zh-TW" />));
  const q = <T extends Element = HTMLElement>(sel: string) => el.querySelector<T>(sel);
  const buttons = () => [...el.querySelectorAll("button")];
  // 結算鈕會帶「（N 人還沒送）」：找不到完全相同的就認開頭
  const button = (label: string) => buttons().find((b) => b.textContent === label) ?? buttons().find((b) => b.textContent?.startsWith(`${label}（`)) ?? null;
  return {
    el, env, hrefs, q, button,
    text: () => el.textContent ?? "",
    click: (label: string) => act(() => button(label)!.click()),
    tab: (k: string) => act(() => q<HTMLButtonElement>(`[data-testid="tab-${k}"]`)!.click()),
    type: (sel: string, value: string) =>
      act(() => {
        const input = q<HTMLInputElement | HTMLTextAreaElement>(sel)!;
        Object.getOwnPropertyDescriptor(Object.getPrototypeOf(input), "value")!.set!.call(input, value);
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
  await hub.exec("gen.part.end", { dispatchId, part: 0, text: "風在吹，酒館門開了。", interrupted: false });
  await until(() => !B.q('[data-testid="streaming"]') && B.text().includes("風在吹，酒館門開了。"));
  await hub.exec("gen.done", { dispatchId, aborted: false });
  await until(() => A.text().includes("輪到大家行動") && B.text().includes("輪到大家行動"));
  return { room, hub, A, B, a: room.s.members[0].id, b: room.s.members[1].id };
}

describe("手機客戶端對真伺服器核心", () => {
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

  it("空房：沒人入座時開始鈕停用並說原因；自建角色名字預填暱稱、執行端建卡後入座就能開始；回合列列出還沒送的人；房主暫停跟離線分開顯示", async () => {
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
    const card = { cardId: "mine", name: "阿明", color: null, avatar: null, blurb: "", playable: true, hidden: false };
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
    await until(() => A.q('[data-testid="round-waiting"]'));
    expect(A.q('[data-testid="round-waiting"]')!.textContent).toBe("還在等：阿明");
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
    expect(A.q('[data-testid="submitted-note"]')!.textContent).toBe("已送出：我拔劍（結算前可以收回修改）"); // 輸入框清空了，寫出送了什麼

    B.click("跳過");
    await until(() => room.s.round?.phase === "queued");
    await until(() => A.text().includes("GM 生成中…") && B.text().includes("GM 生成中…"));
    expect((A.button("收回修改") as HTMLButtonElement).disabled).toBe(true); // 結算後不能收回
    expect(room.events("round.resolved").at(-1)!.payload.by).toBe("all");
    expect(B.q('[data-testid="story"]')!.textContent).toContain("我拔劍"); // 結算後進故事串
  });

  it("房主結算；回合已往下走時送出被拒，草稿原樣保留", async () => {
    const { room, hub, A, B } = await table();
    B.socket().held = true; // B 暫時收不到：畫面停在第 2 回合
    A.click("結算這回合");
    await until(() => room.s.round?.phase === "queued");
    await room.finishGen("第二段");
    hub.flush();
    expect(room.s.round!.round).toBe(3);

    B.type('[data-testid="action-text"]', "我躲到吧台後面");
    B.click("送出");
    await until(() => room.out[B.socket().id].some((f) => f.t === "err" && f.p.code === "E_ROUND_STALE"));
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
    await until(() => B.socket() && B.socket() !== old && B.socket().readyState === 1 && B.text().includes("我推開門"));
    const hello = B.socket().sent[0];
    expect(hello.t).toBe("hello");
    expect(hello.p.lastSeq).toBeGreaterThan(0);
    const welcome = room.out[B.socket().id].find((f) => f.t === "welcome")!;
    expect(welcome.p.snapshot).toBeUndefined(); // 補事件路徑，不是整份快照
    expect(B.q('[data-testid="submitted"]')!.textContent).toContain("我推開門");
  });

  it("換幕：已送出的行動退回草稿、新幕開場帶前情摘要；房主移出成員、關房後畫面唯讀", async () => {
    const { room, hub, A, B } = await table();
    B.type('[data-testid="action-text"]', "我想走");
    B.click("送出");
    await until(() => room.events("action.submitted").length === 1);
    A.tab("host");
    A.click("換幕"); // A 也是出額度者：可同時定下一幕路線／模型（伺服器沒給清單時手填，預設目前這組）
    await until(() => B.text().includes("換幕中，GM 正在整理前情…"));
    expect(B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe("我想走");
    const { jobId } = room.frames("x1", "job.scene_advance")[0].p;
    await hub.exec("job.result", { jobId, ok: true, result: { summary: "前情：大家進了酒館。" } });
    await until(() => B.text().includes("第 2 幕 · 第 3 回合"));
    expect(B.q('[data-testid="story"]')!.textContent).toContain("前情摘要前情：大家進了酒館。");

    const sockets = hub.sockets.length;
    A.click("移除…"); // 入座的人：確認框一起選角色怎麼處理
    act(() => A.q<HTMLInputElement>('[data-testid="remove-confirm"] input[type="radio"]:not(:checked)')!.click()); // 交給 GM 當 NPC
    act(() => [...A.q('[data-testid="remove-confirm"]')!.querySelectorAll("button")].find((x) => x.textContent === "移除")!.click());
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

  it("GM 被擋時全員看到公開說明；出額度者看到細節並從面板重試", async () => {
    const { room, hub, A, B } = await table();
    A.click("結算這回合");
    await until(() => room.s.round?.dispatchId && room.s.round.phase === "queued");
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    const error = { kind: "refusal", code: "AI_CONTENT_FILTERED", message: "供應商拒答，建議切境外路線", actions: ["switch_route", "switch_model", "retry", "continue"], resetsAt: null };
    await hub.exec("gen.failed", { dispatchId, atPart: null, error });
    await until(() => B.text().includes("GM 這段被供應商擋下，等房主處理"));
    expect(B.text()).not.toContain("AI_CONTENT_FILTERED");
    expect(B.text()).toContain("GM 卡住了，房主處理後再送");
    expect(A.text()).toContain("GM 這段被供應商擋下，你可以重試、換模型或跳過"); // A 是出額度者
    expect(B.q('[data-testid="error-detail"]')).toBeNull(); // 細節在回合列，只給出額度者

    await until(() => A.q('[data-testid="error-detail"]'));
    expect(A.q('[data-testid="error-detail"]')!.textContent).toContain("供應商拒答，建議切境外路線");
    // 重新整理（同一台裝置、整份快照）後錯誤細節還在：伺服器在快照帶給出額度者
    const reloaded = phone(hub, `k=${room.joinSecret}`, { storage: A.env.storage });
    await until(() => reloaded.q('[data-testid="tab-payer"]'));
    expect(reloaded.q('[data-testid="error-detail"]')?.textContent).toContain("供應商拒答，建議切境外路線");
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

  it("送出後節流中的草稿不會晚到：伺服器草稿保持清空，下一回合不會把舊行動當草稿", async () => {
    const { room, A, a } = await table();
    A.type('[data-testid="action-text"]', "我");
    await until(() => room.s.drafts[a] === "我");
    A.type('[data-testid="action-text"]', "我拔劍"); // 600 ms 內第二則：排在計時器裡
    A.click("送出");
    await until(() => room.events("action.submitted").length === 1);
    await act(() => new Promise((r) => setTimeout(r, 700)));
    expect(room.s.drafts[a]).toBeUndefined();
    expect(A.socket().sent.filter((f) => f.t === "action.draft").map((f) => f.p.text)).toEqual(["我"]);
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
    expect(B.q('[data-testid="fatal"]')!.textContent).toBe("名字不能空白，最多 24 個字，不能有特殊控制字元");
    const sockets = hub.sockets.length;
    await until(() => true, 5);
    expect(hub.sockets.length).toBe(sockets);
  });

  it("切回前景時偵測到死連線就重連；收不到回覆的請求逾時回報斷線", async () => {
    let now = Date.now();
    const { room, B, b } = await table({ timing: { requestMs: 100, now: () => now } });
    const old = B.socket();
    old.held = true; // 手機熄螢幕：之後什麼都收不到，onclose 也不來
    B.click("跳過");
    await act(() => new Promise((r) => setTimeout(r, 150)));
    await until(() => B.q('[data-testid="action-error"]'));
    expect(B.q('[data-testid="action-error"]')!.textContent).toBe("連線中斷，等重新連上再試");
    expect(room.s.round!.roster.find((e) => e.memberId === b)!.status).toBe("skipped"); // 伺服器其實收到了

    now += 41_000;
    act(() => void document.dispatchEvent(new Event("visibilitychange")));
    await until(() => B.socket() !== old && B.socket()?.readyState === 1 && B.button("收回修改"));
    expect(B.socket().sent[0].p.lastSeq).toBeGreaterThan(0); // 補事件路徑
  });

  it("房主 GM 鈕：只有房主看得到；按旁白時別人已送出的行動回到輸入框；GM 點到的 NPC 只在房主那邊出現「讓 X 接話」", async () => {
    const { room, hub, A, B } = await table();
    expect(A.q('[data-testid="gm-controls"]')).toBeTruthy();
    expect(B.q('[data-testid="gm-controls"]')).toBeNull();
    B.type('[data-testid="action-text"]', "我點酒");
    B.click("送出");
    await until(() => A.q('[data-testid="submitted"]'));
    A.click("GM 旁白");
    await until(() => room.s.round?.kind === "host" && B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value === "我點酒");
    expect(A.q<HTMLButtonElement>('[data-testid="gm-narrate"]')!.disabled).toBe(true);
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    await hub.exec("gen.done", { dispatchId, aborted: false, called: "c3" });
    await until(() => A.button("讓角色3接話"));
    expect(B.button("讓角色3接話")).toBeNull();
    A.click("讓角色3接話");
    await until(() => room.s.round?.host?.call === true);
  });

  it("房主收回上一回合：確認後 GM 段從大家的故事串消失、留一行提示；被收回的行動只回到本人，帶回填進輸入框，不用重連", async () => {
    const { room, hub, A, B } = await table();
    B.type('[data-testid="action-text"]', "我推門");
    B.click("送出");
    A.click("跳過");
    await until(() => room.s.round?.phase === "queued");
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    await hub.exec("gen.part.begin", { dispatchId, part: 0, speaker: { kind: "gm" } });
    await hub.exec("gen.part.end", { dispatchId, part: 0, text: "門後是一片黑。", interrupted: false });
    await hub.exec("gen.done", { dispatchId, aborted: false });
    await until(() => A.button("收回上一回合") && B.text().includes("門後是一片黑。"));
    expect(B.q('[data-testid="undo-controls"]')).toBeNull();
    A.click("收回上一回合");
    await until(() => A.q('[data-testid="undo-confirm"]'));
    A.click("收回上一回合");
    await until(() => room.s.undo);
    await hub.exec("job.result", { jobId: room.s.undo!.jobId, ok: true });
    await until(() => !B.text().includes("門後是一片黑。") && B.text().includes("房主收回了上一回合"));
    expect(B.q('[data-testid="story"]')!.textContent).not.toContain("我推門");
    // 輸入框空著：原文直接放回去、上方一行提示，送出後提示消失；伺服器那份另存清掉
    await until(() => B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value === "我推門" && B.q('[data-testid="returned-notice"]'));
    expect(B.q('[data-testid="returned"]')).toBeNull();
    await until(() => room.s.returned?.[room.s.members[1].id] === undefined);
    B.click("送出");
    await until(() => !B.q('[data-testid="returned-notice"]'));
  });

  it("收回時已經打了新草稿：草稿不動，原文另外顯示，帶回接在草稿後面", async () => {
    const { room, hub, A, B } = await table();
    B.type('[data-testid="action-text"]', "我推門");
    B.click("送出");
    A.click("跳過");
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
    expect(B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe("我想先看看\n我推門");
    expect(room.s.drafts[room.s.members[1].id]).toBe("我想先看看\n我推門");
  });

  it("GM 旁白在第一段前就失敗（held、故事串沒有這回合的段落）：房主照樣看得到收回鈕並能收回", async () => {
    const { room, hub, A } = await table();
    A.click("GM 旁白");
    await until(() => room.s.round?.kind === "host");
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    await hub.exec("gen.failed", { dispatchId, atPart: 0, error: { kind: "model_unavailable", code: "AI_HTTP_STATUS_404", message: "x", actions: ["switch_model", "switch_route", "retry"], resetsAt: null } });
    await until(() => A.button("收回上一回合"));
    // 上次是模型下架：重生的換模型選單灰掉它、預設換成能用的那個
    A.click("重生上一回合");
    await until(() => A.q('[data-testid="undo-confirm"]'));
    const model = A.q<HTMLSelectElement>('[data-testid="undo-confirm"] select[aria-label="模型"]')!;
    expect(model.value).toBe("m-b");
    expect(model.querySelector<HTMLOptionElement>('option[value="m-a"]')!.disabled).toBe(true);
    expect(A.text()).toContain("GM 卡住：模型已下架或叫不到，等房主換模型");
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
        kind: new URL(url, "https://party.example").searchParams.get("kind"), contentLength: String(bytes.length), origin: null, selfOrigin: "https://party.example",
        deviceId: auth.slice("Bearer ".length), body: (async function* () { yield bytes; })(),
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
  const NEW = { cardId: "c9", name: "匯入的", color: null, avatar: null, blurb: "", playable: true, hidden: false };

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
    await hub.exec("job.result", { jobId: last(room, "job.card_import").jobId, ok: false, error: { code: "E_CARD_INVALID" } });
    await until(() => C.text().includes("讀不出這張角色卡"));
    await pickFile(C, sel, new File(['{"name":"匯入的"}'], "ok.json"));
    await until(() => room.frames("x1", "job.card_import").length === 2);
    await hub.exec("roster.update", { cards: [...CARDS, NEW] });
    await hub.exec("job.result", { jobId: last(room, "job.card_import").jobId, ok: true, result: { cardId: "c9" } });
    await until(() => C.q('[data-testid="action-box"]'));
    await until(() => C.q('[data-testid="import-note"]')?.textContent === "匯入完成：只帶入名字、描述和圖");
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
    await hub.exec("job.result", { jobId: room.s.jobs[0].job.jobId, ok: true, result: { cardId: "c9" } });
    await until(() => room.frames("x1", "card.save").length === 1);
    expect(last(room, "card.save")).toMatchObject({ cardId: "c9", privateMd: "其實是公主" });
    expect(last(room, "card.save").publicMd).toBeUndefined();
    await hub.exec("card.save.result", { reqId: last(room, "card.save").reqId, ok: false, code: "E_INTERNAL" });
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
    A.click("編輯我的角色");
    await until(() => room.frames("x1", "card.read").length === 1);
    const card = { name: "角色1", avatar: "🗡", publicMd: "公開", privateMd: "舊秘密", image: null };
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
      await hub.exec("card.read.result", { reqId: last(room, "card.read").reqId, ok: true, card: c });
      await until(() => A.q('[data-testid="card-gen-notice"]'));
    };
    const gen = async (n: number) => {
      A.click("AI 生圖");
      await until(() => room.frames("x1", "image.gen").length === n);
      await hub.exec("image.gen.result", { reqId: last(room, "image.gen").reqId, ok: true, file: "1700.png", preview: "data:image/png;base64,AAAA" });
      await until(() => A.q('[data-testid="card-gen-preview"]'));
    };
    // 「用這張」只進草稿：按返回就撤回，什麼都沒存
    await reopen(2, { ...card, name: "新名字" });
    expect(A.q('[data-testid="card-gen-notice"]')!.textContent).toBe("會用 GM 主機「家裡的電腦」上設定的 AI 額度");
    confirm.mockClear();
    await gen(1);
    expect(confirm).toHaveBeenCalledWith("這會用 GM 主機「家裡的電腦」的 AI 額度生一張圖，要繼續嗎？");
    expect(last(room, "image.gen")).toMatchObject({ cardId: "c1", name: "新名字", description: "公開", framing: "half" });
    expect(A.q('[data-testid="card-gen-preview"]')!.textContent).toContain("新生成的圖");
    // 預覽還沒決定前不能再生，免得以為沒成功又花一次額度
    expect((A.button("AI 生圖") as HTMLButtonElement).disabled).toBe(true);
    // 用這張先過裁切窗，套用才進草稿
    A.click("用這張");
    await until(() => A.q('[data-testid="crop-sheet"]'));
    A.click("套用");
    await until(() => A.q<HTMLImageElement>('[data-testid="card-image-now"]')?.src.startsWith("data:image/png"));
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
    await hub.exec("image.gen.result", { reqId: last(room, "image.gen").reqId, ok: false, code: "E_GEN_QUOTA" });
    await until(() => A.text().includes("GM 主機的 AI 額度用完了"));
    A.click("存檔");
    await until(() => room.frames("x1", "card.save").length === 2);
    const { reqId: r2, memberId: _m2, ...sent2 } = last(room, "card.save");
    expect(sent2).toEqual({ cardId: "c1", imageFromGallery: "1700.png", crop: { x: 0, y: 0, w: 1, h: 1 } });
    await hub.exec("card.save.result", { reqId: r2, ok: true });
    await until(() => !A.q('[data-testid="card-sheet"]'));

    // 主機關掉開關：只留說明，額外提示與構圖一起收起
    await hub.exec("status", { keepAwake: null, busy: false, imageGen: false });
    await reopen(4, card);
    expect(A.q('[data-testid="card-gen-notice"]')!.textContent).toBe("GM 主機關閉了 AI 生圖");
    expect(A.button("AI 生圖")).toBeNull();
    expect(A.q('[data-testid="card-image"] select')).toBeNull();
    // 主機沒有房間能用的生圖路線：整段不出現
    await hub.exec("status", { keepAwake: null, busy: false, imageGen: false, imageGenRoute: false });
    await until(() => !A.q('[data-testid="card-gen-notice"]'));
    // 沒有修改：直接返回，不問
    A.click("返回");
    await until(() => !A.q('[data-testid="card-sheet"]'));
  });

  it("從圖庫選：有圖才出現、按了才撈、標使用中、再載入接著最後一張、點縮圖取大預覽再裁切、取消不動草稿、套用才進草稿、存檔送檔名與裁切框", async () => {
    const { room, hub, A } = await table();
    A.click("編輯我的角色");
    await until(() => room.frames("x1", "card.read").length === 1);
    const card = { name: "角色1", avatar: "🗡", publicMd: "公開", privateMd: "", image: null, gallery: 3, galleryFile: "1700000000002.png" };
    await hub.exec("card.read.result", { reqId: last(room, "card.read").reqId, ok: true, card });
    await until(() => A.button("從圖庫選（3）"));
    expect(room.frames("x1", "card.gallery")).toHaveLength(0);
    A.click("從圖庫選（3）");
    await until(() => room.frames("x1", "card.gallery").length === 1);
    const thumb = "data:image/png;base64,AAAA";
    const page1 = [{ file: "1700000000003.png", thumb }, { file: "1700000000002.png", thumb }];
    await hub.exec("card.gallery.result", { reqId: last(room, "card.gallery").reqId, ok: true, items: page1, more: 1 });
    await until(() => A.q(".card-gallery-thumb"));
    expect(A.q('[data-testid="card-gallery"]')!.textContent).toContain("使用中");
    A.click("載入更多（剩 1 張）");
    await until(() => room.frames("x1", "card.gallery").length === 2);
    expect(last(room, "card.gallery")).toMatchObject({ before: "1700000000002.png" });
    await hub.exec("card.gallery.result", { reqId: last(room, "card.gallery").reqId, ok: true, items: [{ file: "1700000000001.png", thumb }], more: 0 });
    await until(() => A.el.querySelectorAll(".card-gallery-thumb").length === 3);
    // 點縮圖：取單張預覽 → 裁切窗；取消就什麼都不變
    const thumbs = () => [...A.el.querySelectorAll<HTMLButtonElement>(".card-gallery-thumb")];
    act(() => thumbs()[2].click());
    await until(() => room.frames("x1", "card.gallery").length === 3);
    expect(last(room, "card.gallery")).toMatchObject({ file: "1700000000001.png" });
    expect(thumbs().every((b) => b.disabled)).toBe(true); // 取單張進行中不重複送
    await hub.exec("card.gallery.result", { reqId: last(room, "card.gallery").reqId, ok: true, items: [{ file: "1700000000001.png", thumb }] });
    await until(() => A.q('[data-testid="crop-sheet"]'));
    A.click("取消");
    await until(() => !A.q('[data-testid="crop-sheet"]'));
    expect(A.q('[data-testid="card-image-now"]')).toBeNull();
    act(() => thumbs()[2].click());
    await until(() => room.frames("x1", "card.gallery").length === 4);
    await hub.exec("card.gallery.result", { reqId: last(room, "card.gallery").reqId, ok: true, items: [{ file: "1700000000001.png", thumb }] });
    await until(() => A.q('[data-testid="crop-sheet"]'));
    // 套用後草稿還在做（切預覽慢）：裁切窗不關、兩顆鈕停用，編卡的存檔與返回也停用
    let finish = () => {};
    vi.stubGlobal("fetch", () => new Promise<Response>((r) => (finish = () => r(new Response(new Blob())))));
    A.click("套用");
    await until(() => A.button("套用")?.disabled);
    expect((A.button("取消") as HTMLButtonElement).disabled).toBe(true);
    expect((A.button("存檔") as HTMLButtonElement).disabled).toBe(true);
    expect((A.button("返回") as HTMLButtonElement).disabled).toBe(true);
    expect(A.q('[data-testid="card-image-chosen"]')).toBeNull();
    await act(async () => finish());
    await until(() => !A.q('[data-testid="crop-sheet"]') && A.q('[data-testid="card-image-chosen"]'));
    expect((A.button("存檔") as HTMLButtonElement).disabled).toBe(false);
    expect(thumbs()[2].className).toContain("picked");
    A.click("存檔");
    await until(() => room.frames("x1", "card.save").length === 1);
    const { reqId: _r, memberId: _m, ...sent } = last(room, "card.save");
    expect(sent).toEqual({ cardId: "c1", imageFromGallery: "1700000000001.png", crop: { x: 0, y: 0, w: 1, h: 1 } });
  });
});

describe("AI 代管（room-seat-delegate 包 3）", () => {
  const inside = (p: Phone, testId: string, label: string) => act(() => [...p.q(`[data-testid="${testId}"]`)!.querySelectorAll("button")].find((x) => x.textContent === label)!.click());

  it("已送出後自己交給 AI 代管：先確認，原文回到輸入框、常駐列出現、送出停用；別人名單看到標籤；接回後收起", async () => {
    const { room, A, B, b } = await table();
    B.type('[data-testid="action-text"]', "我先走");
    B.click("送出");
    await until(() => room.events("action.submitted").length === 1);
    B.tab("members");
    B.click("交給 AI 代管");
    expect(B.q('[data-testid="delegate-confirm"]')!.textContent).toContain("你這回合已送出的行動會退回草稿、不計入");
    inside(B, "delegate-confirm", "交給 AI 代管");
    await until(() => B.q('[data-testid="delegated-banner"]'));
    expect(B.q('[data-testid="delegated-banner"]')!.textContent).toContain("接回後從下一回合開始行動");
    B.tab("story");
    expect(B.q<HTMLTextAreaElement>('[data-testid="action-text"]')!.value).toBe("我先走");
    expect(B.text()).toContain("你的角色代管中，接回後才能送行動");
    expect(B.button("送出")!.disabled).toBe(true);
    A.tab("members");
    await until(() => A.q(`[data-testid="member-${b}"]`)!.textContent!.includes("AI 代管中"));
    B.click("接回");
    await until(() => !B.q('[data-testid="delegated-banner"]'));
    expect(room.s.members.find((m) => m.id === b)!.delegated).toBe(false);
  });

  it("暫時離開：遊戲中已入座跳兩鈕確認，「交給 AI 代管再離開」先代管再斷線", async () => {
    const { room, B, b } = await table();
    B.tab("members");
    B.click("暫時離開");
    expect(B.q('[data-testid="leave-confirm"]')!.textContent).toContain("你的座位會保留。要不要先把角色交給 AI 代管？");
    inside(B, "leave-confirm", "交給 AI 代管再離開");
    await until(() => B.text().includes("你已暫時離開"));
    expect(room.s.members.find((m) => m.id === b)!.delegated).toBe(true);
  });

  it("房主：「還在等」裡離線的人旁邊直接代管；全員代管時房主與玩家看到不同的原因，接回寫馬上能行動", async () => {
    const { room, A, B, b } = await table();
    B.tab("members");
    B.click("暫時離開");
    inside(B, "leave-confirm", "直接離開（座位保留）");
    await until(() => A.q(`[data-testid="waiting-delegate-${b}"]`));
    act(() => A.q<HTMLButtonElement>(`[data-testid="waiting-delegate-${b}"]`)!.click());
    await until(() => room.s.members.find((m) => m.id === b)!.delegated);

    const { room: r2, A: A2, B: B2, a: a2, b: b2 } = await table();
    A2.tab("host");
    act(() => A2.q<HTMLButtonElement>(`[data-testid="attend-delegate-${b2}"]`)!.click());
    await until(() => r2.s.members.find((m) => m.id === b2)!.delegated);
    act(() => A2.q<HTMLButtonElement>(`[data-testid="attend-delegate-${a2}"]`)!.click());
    await until(() => A2.text().includes("全員代管中，回合不會自動進行：按 GM 推進，或取消某人的代管"));
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
    expect(A.q('[data-testid="delegate-offline"]')!.textContent).toBe("把沒連線的人都交給 AI 代管（1 人）");
    act(() => A.q<HTMLButtonElement>('[data-testid="delegate-offline"]')!.click());
    await until(() => !A.q('[data-testid="delegate-offline"]'));
    expect(A.q(`[data-testid="host-${b}"]`)!.textContent).toContain("AI 代管中");

    A.click("邀請新朋友");
    await until(() => A.q('[data-testid="invite"]'));
    expect(A.q<HTMLInputElement>('[data-testid="invite"]')!.value).toBe(`https://party.example/r/${room.code}#k=${room.joinSecret}`);
    expect(room.frames("x1", "invite")).toEqual([]);

    A.click("暫停房間");
    expect(A.q('[data-testid="pause-confirm"]')!.textContent).toContain("得在電腦（");
    inside(A, "pause-confirm", "暫停房間");
    await until(() => room.frames("x1", "ctl").length === 1);
    expect(room.frames("x1", "ctl")[0].p.cmd).toEqual({ kind: "pause" });
    expect(A.q<HTMLButtonElement>('[data-testid="pause-room"]')!.disabled).toBe(true);
  });
});
