// 信任邊界（協定 §10）：訊框外形與欄位內容的純函式檢查。失敗一律由呼叫端回錯誤碼、狀態不動。
import type { ErrorCode, ExecutorError, Frame, PublicCard, QuotaReport, Capabilities, Speaker, StoryPart, SyncItem } from "./types";

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
export const isCount = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;
const TYPES: Record<string, (v: unknown) => boolean> = {
  s: (v) => typeof v === "string",
  i: isCount,
  b: (v) => typeof v === "boolean",
  o: isObj,
  a: Array.isArray,
  n: (v) => typeof v === "number" && Number.isFinite(v),
};

/** `"round:i text:s next?:o atPart:i~"`：`?`＝可省、`~`＝可為 null；多的欄位忽略 */
export function shape(v: unknown, spec: string): v is Record<string, any> {
  if (!isObj(v)) return false;
  return spec.split(" ").filter(Boolean).every((field) => {
    const [k, t] = field.split(":");
    const key = k.replace("?", "");
    const val = v[key];
    if (val === undefined) return k.endsWith("?");
    if (val === null) return t.endsWith("~");
    return TYPES[t.replace("~", "")](val);
  });
}

const MEMBER_FRAMES: Record<string, string> = {
  hello: "protocol:i role:s deviceId:s joinSecret?:s nickname?:s pairCode?:s lastSeq:i client:o",
  "profile.rename": "nickname:s", "seat.claim": "cardId:s", "seat.release": "", "seat.create": "name:s blurb:s",
  "action.draft": "text:s", "action.submit": "round:i text:s", "action.skip": "round:i", "action.retract": "round:i",
  "round.resolve": "round:i", "round.continue": "round:i", "gen.abort": "round:i", "host.gm": "round:i mode:s cardId?:s", "round.undo": "round:i mode:s next?:o", "action.returned_dismiss": "draft?:s", "room.start": "",
  "room.settings": "patch:o", "invite.regenerate": "", "host.transfer": "memberId:s", "member.kick": "memberId:s ban:b",
  "member.departure": "cardId:s mode:s", "member.reassign": "from:s to:s", "member.leave": "",
  "scene.advance": "round:i next?:o", "scene.advance_cancel": "", "room.close": "", "pair.claim": "code:s",
  "pair.leave": "", ctl: "ctlId:s cmd:o", "history.page": "beforeSeq:i limit:i", ping: "",
  "card.mine": "", "card.save": "cardId:s name?:s avatar?:s publicMd?:s privateMd?:s removeImage?:b imageFromGallery?:s crop?:o",
  "card.gallery": "before?:s file?:s",
  "image.gen": "cardId:s name:s description:s extraPrompt:s framing:s",
};
const EXECUTOR_FRAMES: Record<string, string> = {
  hello: "protocol:i role:s roomCode:s executorToken:s lastSeq:i inflight:o~ client:o paused?:b", hb: "",
  capabilities: "routes:a current:o maxRoundSpeakers:i", "room.seed": "part:i total:i cards?:a history?:a", "story.sync": "part:i total:i keepThrough?:o~ items?:a",
  "roster.update": "cards:a", "seat.reserve": "cardId:s~ deviceId:s", "invite.ack": "joinSecret:s", "dispatch.ack": "dispatchId:s", "gen.part.begin": "dispatchId:s part:i speaker:o",
  "gen.delta": "dispatchId:s part:i text:s", "gen.part.end": "dispatchId:s part:i text:s interrupted:b",
  "gen.done": "dispatchId:s aborted:b cardsArrived?:a called?:s", "gen.failed": "dispatchId:s atPart:i~ error:o",
  "job.ack": "jobId:s", "job.progress": "jobId:s", "job.result": "jobId:s ok:b result?:o error?:o",
  "pair.create": "grantHost:b", "pair.revoke": "", status: "keepAwake:b~ busy:b paused?:b imageGen?:b imageGenRoute?:b",
  quota: "route:s routeLabel:s window:s state:s usedRatio:n~ resetsAt:i~ creditsLeft:s~ observedAt:i",
  "ctl.result": "ctlId:s ok:b error?:o", "executor.leave": "",
  "card.read.result": "reqId:s ok:b card?:o code?:s", "card.save.result": "reqId:s ok:b code?:s",
  "image.gen.result": "reqId:s ok:b file?:s preview?:s code?:s",
  "card.gallery.result": "reqId:s ok:b items?:a more?:i code?:s",
};
const LIMIT = { member: 16 * 1024, executor: 1024 * 1024 };
/** 成員訊框只有 `card.save` 放寬（兩段各 64,000 字）；不分片（room-player-cards §6-2） */
const MEMBER_WIDE: Record<string, number> = { "card.save": 512 * 1024 };

function wellFormed(v: unknown): boolean {
  if (typeof v === "string") return (v as string & { isWellFormed(): boolean }).isWellFormed();
  if (Array.isArray(v)) return v.every(wellFormed);
  if (isObj(v)) return Object.entries(v).every(([k, x]) => wellFormed(k) && wellFormed(x));
  return true;
}

export function validateFrame(raw: string, from: "member" | "executor"): { ok: true; frame: Frame } | { ok: false; code: ErrorCode } {
  const bad = { ok: false as const, code: "E_BAD_FRAME" as const };
  const max = from === "member" ? MEMBER_WIDE["card.save"] : LIMIT[from];
  const size = raw.length > max ? Infinity : new TextEncoder().encode(raw).length;
  if (size > max) return bad;
  let v: unknown;
  try {
    v = JSON.parse(raw);
  } catch {
    return bad;
  }
  if (!isObj(v) || typeof v.t !== "string" || !wellFormed(v)) return bad;
  const table = from === "member" ? MEMBER_FRAMES : EXECUTOR_FRAMES;
  if (!Object.hasOwn(table, v.t)) return bad;
  if (from === "member" && size > (MEMBER_WIDE[v.t] ?? LIMIT.member)) return bad;
  if (v.id !== undefined && (typeof v.id !== "string" || !/^[A-Za-z0-9_-]{1,36}$/.test(v.id))) return bad;
  const p = v.p ?? {};
  if (!shape(p, table[v.t])) return bad;
  return { ok: true, frame: { t: v.t, ...(v.id !== undefined ? { id: v.id as string } : {}), p } };
}

// ── 文字規則（§10） ──
const chars = (s: string) => [...s].length;
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;
const FORMAT_BUT_ZERO_WIDTH = /(?![​-‍])\p{Cf}/u;

/** 暱稱與卡名：NFC、去頭尾空白、1–max 字元、無控制／雙向覆寫／非零寬的格式字元。不合回 null */
export function cleanName(v: unknown, max = 24): string | null {
  if (typeof v !== "string") return null;
  const s = v.normalize("NFC").trim();
  return chars(s) >= 1 && chars(s) <= max && !CONTROL.test(s) && !FORMAT_BUT_ZERO_WIDTH.test(s) ? s : null;
}

/** 行動、草稿、描述：NFC、`\r\n` 正規化、min–max 字元、最多 30 行、只允許 `\n` 這一種控制字元 */
export function cleanText(v: unknown, min: number, max = 2000): string | null {
  if (typeof v !== "string") return null;
  const s = v.replace(/\r\n/g, "\n").normalize("NFC");
  const ok = chars(s) >= min && chars(s) <= max && s.split("\n").length <= 30 && !/[\u0000-\u0009\u000b-\u001f\u007f-\u009f]/.test(s);
  return ok ? s : null;
}

/** 卡片長文（公開段、秘密段、生圖描述）：NFC、`\r\n` 正規化、0–max 字元、控制字元只允許 `\n` 與 `\t`，不限行數 */
export function cleanLong(v: unknown, max: number): string | null {
  if (typeof v !== "string") return null;
  const s = v.replace(/\r\n/g, "\n").normalize("NFC");
  return chars(s) <= max && !/[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/.test(s) ? s : null;
}
/** 圖示（emoji）：1–16 字元、沒有空白與控制字元 */
export const cleanIcon = (v: unknown) => (typeof v === "string" && chars(v) >= 1 && chars(v) <= 16 && !/[\s\u0000-\u001f\u007f-\u009f]/.test(v) ? v : null);
/** 圖庫檔名（桌面版 `<毫秒>.png`）：不含路徑與 `..`，執行端再對那張卡的圖庫驗一次 */
export const GALLERY_FILE = /^(?!.*\.\.)[A-Za-z0-9_-][A-Za-z0-9_.-]{0,79}\.png$/;

/** 裁切框（原圖比例 0–1，room-phone-gallery §3.5）：四欄都要是有限數字、在原圖範圍內（容差 0.0001）。不合回 null */
export function cleanCrop(v: unknown): { x: number; y: number; w: number; h: number } | null {
  if (!shape(v, "x:n y:n w:n h:n")) return null;
  const { x, y, w, h } = v;
  return x >= 0 && y >= 0 && w > 0 && h > 0 && x + w <= 1.0001 && y + h <= 1.0001 ? { x, y, w, h } : null;
}

const str = (v: unknown, max: number, min = 0) => typeof v === "string" && v.length >= min && chars(v) <= max;
export const ID64 = /^[A-Za-z0-9_-]{1,64}$/;
const AVATAR = /^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/]+={0,2}$/;

/** 執行端上送的名冊：≤ 64 張、欄位照 §10；`claimedBy` 一律由伺服器填 */
export function cleanCards(list: unknown): PublicCard[] | null {
  if (!Array.isArray(list) || list.length > 64) return null;
  const out: PublicCard[] = [];
  for (const c of list) {
    const ok =
      shape(c, "cardId:s name:s color:s~ avatar:s~ blurb:s playable:b hidden:b") &&
      ID64.test(c.cardId) && str(c.name, 64, 1) && str(c.blurb, 1000) &&
      (c.color === null || /^#[0-9a-fA-F]{6}$/.test(c.color)) &&
      (c.avatar === null || (c.avatar.length <= 65536 && AVATAR.test(c.avatar))) &&
      !out.some((o) => o.cardId === c.cardId);
    if (!ok) return null;
    out.push({ cardId: c.cardId, name: c.name, color: c.color, avatar: c.avatar, blurb: c.blurb, playable: c.playable, hidden: c.hidden, claimedBy: null });
  }
  return out;
}

const MARKERS = ["scene_summary", "card_arrival", "person_arrival", "state_update", "solo_before", "solo_paused", "room_reopened"];
export function cleanSpeaker(v: unknown, cards: PublicCard[]): Speaker | null {
  if (!shape(v, "kind:s")) return null;
  if (v.kind === "gm") return { kind: "gm" };
  if (v.kind === "system") return MARKERS.includes(v.marker) ? { kind: "system", marker: v.marker } : null;
  const known = shape(v, "cardId:s name:s") && str(v.name, 64, 1) && cards.some((c) => c.cardId === v.cardId);
  if (known && v.kind === "npc") return { kind: "npc", cardId: v.cardId, name: v.name };
  if (known && v.kind === "player" && typeof v.memberId === "string") return { kind: "player", memberId: v.memberId, cardId: v.cardId, name: v.name };
  return null;
}

export function cleanHistory(list: unknown, cards: PublicCard[]): StoryPart[] | null {
  if (!Array.isArray(list)) return null;
  const out: StoryPart[] = [];
  for (const h of list) {
    const speaker = shape(h, "speaker:o text:s interrupted:b at:i") && str(h.text, 64000) ? cleanSpeaker(h.speaker, cards) : null;
    if (!speaker) return null;
    out.push({ round: 0, part: out.length, speaker, text: h.text, interrupted: h.interrupted, at: h.at });
  }
  return out;
}

/** `story.sync` 的項目：段落照 `room.seed` 的驗法，摘要 ≤ 64,000 字 */
export function cleanSyncItems(list: unknown, cards: PublicCard[]): SyncItem[] | null {
  if (!Array.isArray(list)) return null;
  const out: SyncItem[] = [];
  for (const x of list) {
    if (shape(x, "summary:s") && str(x.summary, 64000)) out.push({ summary: x.summary });
    else {
      const part = shape(x, "part:o") ? cleanHistory([x.part], cards)?.[0] : null;
      if (!part) return null;
      out.push({ part });
    }
  }
  return out;
}

const ERROR_KINDS = ["refusal", "quota", "tool_format", "provider_unreachable", "scene_full", "not_ready", "model_unavailable", "other"];
const ACTIONS = ["retry", "switch_model", "switch_route", "continue", "scene_advance"];
export function cleanError(v: unknown): ExecutorError | null {
  const ok =
    shape(v, "kind:s code:s message:s actions:a resetsAt:i~") && ERROR_KINDS.includes(v.kind) &&
    str(v.code, 64) && str(v.message, 500) && v.actions.every((a: unknown) => ACTIONS.includes(a as string));
  return ok ? { kind: v.kind, code: v.code, message: v.message, actions: [...new Set<any>(v.actions)], resetsAt: v.resetsAt } : null;
}

export function cleanQuota(v: Record<string, any>): QuotaReport | null {
  const ok =
    str(v.route, 64) && str(v.routeLabel, 64) && ["rolling", "daily", "weekly", "credits", "unknown"].includes(v.window) &&
    ["ok", "near", "exhausted", "unknown"].includes(v.state) && (v.usedRatio === null || (v.usedRatio >= 0 && v.usedRatio <= 1)) &&
    (v.creditsLeft === null || str(v.creditsLeft, 40));
  const { route, routeLabel, window, state, usedRatio, resetsAt, creditsLeft, observedAt } = v;
  return ok ? { route, routeLabel, window, state, usedRatio, resetsAt, creditsLeft, observedAt } : null;
}

export function cleanCapabilities(v: Record<string, any>): Capabilities | null {
  const routes = v.routes as unknown[];
  const ok =
    routes.length <= 16 &&
    routes.every((r) => shape(r, "route:s label:s models:a") && str(r.route, 64, 1) && str(r.label, 64) && r.models.length <= 200 &&
      r.models.every((m: unknown) => shape(m, "id:s label:s laxity:i~") && str(m.id, 200, 1) && str(m.label, 200) && [1, 2, 3, null].includes(m.laxity))) &&
    shape(v.current, "route:s model:s modelLabel:s") && str(v.current.route, 64, 1) && str(v.current.model, 200, 1) && str(v.current.modelLabel, 200);
  if (!ok) return null;
  const { route, model, modelLabel } = v.current;
  return { routes: routes as Capabilities["routes"], current: { route, model, modelLabel }, maxRoundSpeakers: v.maxRoundSpeakers };
}
