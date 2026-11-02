// 邀請連結與本機身分（協定 §1.2）：`/r/<roomCode>#k=<joinSecret>[&p=<pairCode>]`；deviceId 存 localStorage、跨房間共用一份。
// localStorage 讀寫一律 try/catch：存不了時 deviceId 只在這次有效（重新整理就是新成員，由房主 member.reassign 移座位）。

export interface Invite {
  code: string;
  joinSecret: string | null;
  pairCode: string | null;
}

/** 解析網址；不是房間頁回 null。密鑰在片段（#），不進伺服器存取紀錄與 Referer */
export function parseInvite(href: string): Invite | null {
  const url = new URL(href);
  const m = url.pathname.match(/^\/r\/([0-9A-Za-z]{8})\/?$/);
  if (!m) return null;
  const hash = new URLSearchParams(url.hash.replace(/^#/, ""));
  return { code: m[1].toUpperCase(), joinSecret: hash.get("k"), pairCode: hash.get("p") };
}

/** 配對碼一次性：加入後從網址拿掉，免得重新整理再送一次、累積全房錯誤次數 */
export const withoutPairCode = (href: string) => href.replace(/([#&])p=[^&]*&?/, "$1").replace(/[#&]$/, "");

export const isWeChat = (ua: string) => /MicroMessenger/i.test(ua);

const DEVICE = "tt-party:device";
const ROOMS = "tt-party:rooms";

function read(storage: Storage | null, key: string): string | null {
  try {
    return storage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}
function write(storage: Storage | null, key: string, value: string) {
  try {
    storage?.setItem(key, value);
  } catch {
    // 存不了就只在這次有效
  }
}

export function deviceId(storage: Storage | null): string {
  const stored = read(storage, DEVICE);
  if (stored && /^[A-Za-z0-9_-]{22}$/.test(stored)) return stored;
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  const id = btoa(String.fromCharCode(...bytes)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  write(storage, DEVICE, id);
  return id;
}

/** 這台裝置加入過哪些房間（房間碼 → 暱稱）：加入過的直接重連，不再問暱稱 */
export function joinedNickname(storage: Storage | null, code: string): string | null {
  try {
    return (JSON.parse(read(storage, ROOMS) ?? "{}") as Record<string, string>)[code] ?? null;
  } catch {
    return null;
  }
}
export function rememberJoined(storage: Storage | null, code: string, nickname: string) {
  let rooms: Record<string, string> = {};
  try {
    rooms = JSON.parse(read(storage, ROOMS) ?? "{}");
  } catch {
    // 壞掉就重寫
  }
  write(storage, ROOMS, JSON.stringify({ ...rooms, [code]: nickname }));
}
