// @vitest-environment happy-dom
// 手機客戶端整合測試：真的伺服器核心（room-test-kit）當假伺服器，兩支「手機」各自渲染 App，透過假 WebSocket 連進同一房。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { MemoryStorage } from "@web/features/openrouter/memory-storage";
import { CARDS, Room, STAGE } from "../../../server/core/room-test-kit";
import App from "../../App";
import { applyLang } from "../../i18n";
import { Hub, type FakeSocket } from "./fake-server";
import type { RoomEnv } from "./useRoom";

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
  const button = (label: string) => [...el.querySelectorAll("button")].find((b) => b.textContent === label) ?? null;
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
  await hub.exec("gen.part.end", { dispatchId, part: 0, text: "風在吹，酒館門開了。", interrupted: false });
  await until(() => !B.q('[data-testid="streaming"]') && B.text().includes("風在吹，酒館門開了。"));
  await hub.exec("gen.done", { dispatchId, aborted: false });
  await until(() => A.text().includes("輪到大家行動") && B.text().includes("輪到大家行動"));
  return { room, hub, A, B, a: room.s.members[0].id, b: room.s.members[1].id };
}

describe("手機客戶端對真伺服器核心", () => {
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

    B.click("跳過");
    await until(() => room.s.round?.phase === "queued");
    await until(() => A.text().includes("GM 生成中…") && B.text().includes("GM 生成中…"));
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
    const { room, hub, A, B, b } = await table();
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
    act(() => A.q(`[data-testid="host-${b}"]`)!.querySelectorAll("button")[1].click()); // 移出
    await until(() => B.q('[data-testid="fatal"]'));
    expect(B.q('[data-testid="fatal"]')!.textContent).toBe("你已被移出房間");
    await until(() => true, 3);
    expect(hub.sockets.length).toBe(sockets); // 被移出不自動重連

    vi.stubGlobal("confirm", () => true);
    A.click("關閉房間");
    await until(() => A.text().includes("房間已結束"));
    expect(A.q('[data-testid="story"]') ?? A.q('[data-testid="host-panel"]')).toBeTruthy(); // 留著畫面，房主還能匯出
  });

  it("GM 被擋時全員看到公開說明；出額度者看到細節並從面板重試", async () => {
    const { room, hub, A, B } = await table();
    A.click("結算這回合");
    await until(() => room.s.round?.dispatchId && room.s.round.phase === "queued");
    const dispatchId = room.dispatchId();
    await hub.exec("dispatch.ack", { dispatchId });
    const error = { kind: "refusal", code: "AI_CONTENT_FILTERED", message: "供應商拒答，建議切境外路線", actions: ["switch_route", "switch_model", "retry", "continue"], resetsAt: null };
    await hub.exec("gen.failed", { dispatchId, atPart: null, error });
    await until(() => B.text().includes("GM 這段被供應商擋下，等出額度者處理"));
    expect(B.text()).not.toContain("AI_CONTENT_FILTERED");

    A.tab("payer");
    await until(() => A.q('[data-testid="error-detail"]'));
    expect(A.q('[data-testid="error-detail"]')!.textContent).toContain("供應商拒答，建議切境外路線");
    // 重新整理（同一台裝置、整份快照）後錯誤細節還在：伺服器在快照帶給出額度者
    const reloaded = phone(hub, `k=${room.joinSecret}`, { storage: A.env.storage });
    await until(() => reloaded.q('[data-testid="tab-payer"]'));
    reloaded.tab("payer");
    expect(reloaded.q('[data-testid="error-detail"]')?.textContent).toContain("供應商拒答，建議切境外路線");
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
});
