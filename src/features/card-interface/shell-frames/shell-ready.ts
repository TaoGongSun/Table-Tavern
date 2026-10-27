// 殼的 ready 握手（.ai/plans/shell-update-flash.md）：沙盒 iframe 的 load 事件在 WKWebView 上靠不住，
// 改由殼內墊片主動 postMessage 說「我畫得出來了」，宿主收到才翻面。另附量測用的時間戳與時鐘校準 pong。

/** ready 的成因：raf＝DOMContentLoaded 後兩次 rAF；timer＝頁面可見但 rAF 300 ms 內沒跑 */
export type ReadyReason = "raf" | "timer";

export interface ReadyMessage {
  token: string;
  reason: ReadyReason;
  /** 殼內時刻（timeOrigin＋now，跨 frame 可比）：dcl、第二次 rAF（只有 reason=raf 才有）、送出 */
  t: { dcl: number | null; raf: number | null; sent: number };
}

/**
 * 殼內墊片原始碼（純 JS）。必須排在橋接墊片覆寫 window.parent 之前執行：真 parent 在這裡先收進閉包。
 * 卡片可能改寫 requestAnimationFrame／setTimeout，這裡先收好原生的。
 */
export function buildReadySignalSource(tokenLiteral: string): string {
  return `
(function () {
  var parentRef = window.parent;
  var TOKEN = ${tokenLiteral};
  var raf = window.requestAnimationFrame.bind(window);
  var later = window.setTimeout.bind(window);
  var stop = window.clearTimeout.bind(window);
  function now() {
    return performance.timeOrigin + performance.now();
  }
  function post(message) {
    message.source = "table-tavern-card";
    message.token = TOKEN;
    try {
      parentRef.postMessage(message, "*");
    } catch (error) {
      console.warn("[table-tavern] 無法回報宿主", error);
    }
  }
  var t = { dcl: null, raf: null, sent: 0 };
  var sent = false;
  function send(reason) {
    if (sent) return;
    sent = true;
    t.sent = now();
    post({ kind: "ready", reason: reason, t: t });
  }
  function arm() {
    if (document.visibilityState === "hidden") {
      document.addEventListener("visibilitychange", function wake() {
        if (document.visibilityState === "hidden") return;
        document.removeEventListener("visibilitychange", wake);
        arm();
      });
      return;
    }
    var ran = false;
    var timer = later(function () {
      if (!ran) send("timer");
    }, 300);
    raf(function () {
      ran = true;
      raf(function () {
        stop(timer);
        t.raf = now();
        send("raf");
      });
    });
  }
  document.addEventListener("DOMContentLoaded", function () {
    t.dcl = now();
    arm();
  });
  // 時鐘校準：宿主送 ping（帶宿主時刻），原樣帶回並附殼內時刻
  window.addEventListener("message", function (event) {
    if (event.source !== parentRef) return;
    var data = event.data;
    if (!data || data.source !== "table-tavern-host" || data.kind !== "ping" || data.token !== TOKEN) return;
    post({ kind: "pong", t0: data.t0, t1: now() });
  });
  // 首次內容繪製（引擎有提供才有），只供量測
  try {
    new PerformanceObserver(function (list) {
      list.getEntries().forEach(function (entry) {
        if (entry.name === "first-contentful-paint") post({ kind: "paint", fcp: performance.timeOrigin + entry.startTime });
      });
    }).observe({ type: "paint", buffered: true });
  } catch (error) {
    // 沒有 Paint Timing 就算了
  }
})();
`;
}

function finiteOrNull(value: unknown): value is number | null {
  return value === null || (typeof value === "number" && Number.isFinite(value));
}

/** 收到的 ready 形狀檢查；不對回 null */
export function parseReady(data: unknown): ReadyMessage | null {
  if (typeof data !== "object" || data === null) return null;
  const message = data as Record<string, unknown>;
  if (message.source !== "table-tavern-card" || message.kind !== "ready" || typeof message.token !== "string") return null;
  if (message.reason !== "raf" && message.reason !== "timer") return null;
  const t = message.t as Record<string, unknown> | null;
  if (typeof t !== "object" || t === null || !finiteOrNull(t.dcl) || !finiteOrNull(t.raf) || typeof t.sent !== "number") return null;
  return { token: message.token, reason: message.reason, t: { dcl: t.dcl, raf: t.raf, sent: t.sent } };
}

/** 跨 frame 可比的宿主時刻 */
export function hostNow(): number {
  return performance.timeOrigin + performance.now();
}

/**
 * 量測紀錄：只有測試通道事先建好 window.__ttShellTiming 陣列才寫，正式使用不留任何東西。
 */
export function recordShellTiming(entry: Record<string, unknown>): void {
  const log = (window as { __ttShellTiming?: unknown }).__ttShellTiming;
  if (Array.isArray(log)) log.push({ at: hostNow(), ...entry });
}

export function shellTimingEnabled(): boolean {
  return Array.isArray((window as { __ttShellTiming?: unknown }).__ttShellTiming);
}
