// 玩家卡（room-player-cards §3.1–§3.4、§3.6）：上傳兩段式（先占名額、再掛本體）、執行端取檔、讀卡／存卡／生圖轉送。
// 上傳本體、秘密段、生成圖只經過 live 與訊框，不進 RoomState、事件、快照與匯出；工作列（w.jobs）只帶 id。
import { claim, rate, type Handler } from "./member";
import { MIN } from "./round";
import { sameHash, sha256Hex, ulid } from "./secrets";
import { Fail, need, type Tx } from "./tx";
import type { CardReq, CoreInput, ErrorCode, Job, Member, RoomState, Upload } from "./types";
import { cleanCrop, cleanIcon, cleanLong, cleanName, GALLERY_FILE, isCount, shape } from "./validate";

const MiB = 1 << 20;
export const UPLOAD_MAX = { card: 8 * MiB, image: 2 * MiB };
const UPLOAD_TTL = 10 * MIN;
const REQ_TTL: Record<CardReq["kind"], number> = { mine: 30_000, save: 30_000, gen: 10 * MIN, gallery: 30_000 };
const PNG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const PNG_URL = /^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/;
const EXEC_CODES = new Set<ErrorCode>([
  "E_FORBIDDEN", "E_STATE_CONFLICT", "E_TEXT_INVALID", "E_NAME_INVALID", "E_TOO_LARGE", "E_UNSUPPORTED_FORMAT",
  "E_CARD_INVALID", "E_GEN_QUOTA", "E_GEN_UNSUPPORTED", "E_GEN_REFUSED",
]);
/** 執行端回的錯誤只認白名單碼，不轉任何檔案內容或解析細節 */
const execCode = (v: unknown): ErrorCode => (EXEC_CODES.has(v as ErrorCode) ? (v as ErrorCode) : "E_INTERNAL");
type UploadJob = Extract<Job, { kind: "card_import" | "card_image" }>;
const isUploadJob = (j: Job): j is UploadJob => j.kind === "card_import" || j.kind === "card_image";

export async function memberByDevice(s: RoomState, deviceId: string): Promise<Member | undefined> {
  if (!/^[A-Za-z0-9_-]{22}$/.test(deviceId)) return undefined;
  const dev = await sha256Hex(`${s.code}:${deviceId}`);
  return s.members.find((m) => sameHash(m.dev, dev));
}
const seatedOn = (s: RoomState, memberId: string, cardId: string | null) => s.members.some((m) => m.id === memberId && !m.banned && m.cardId === cardId);

function signatureOk(kind: Upload["kind"], b: Uint8Array) {
  if (PNG.every((x, i) => b[i] === x)) return true;
  if (kind === "image") return false;
  let i = b[0] === 0xef && b[1] === 0xbb && b[2] === 0xbf ? 3 : 0;
  while (i < b.length && (b[i] === 0x20 || b[i] === 0x09 || b[i] === 0x0a || b[i] === 0x0d)) i++;
  return b[i] === 0x7b; // {
}

/** 結束一件上傳（成功、失敗、到期、執行端離線）：收回名額、撤工作、只通知上傳者 */
function finishUpload(tx: Tx, memberId: string, uploadId: string, result: { ok: true; cardId: string } | { ok: false; code: ErrorCode }) {
  delete tx.ctx.live.uploads[uploadId];
  // 用 uploadId 找工作：DO 被逐出後 live 已空，工作還留在 w.jobs
  const jobId = tx.s.jobs.find((j) => isUploadJob(j.job) && j.job.uploadId === uploadId)?.job.jobId;
  if (jobId) {
    const w = tx.w;
    w.jobs = w.jobs.filter((j) => j.job.jobId !== jobId);
    w.doneJobs = [...w.doneJobs.slice(-19), jobId];
  }
  tx.toMember(memberId, "vol", { type: "card_upload", uploadId, ...result });
}

/** 第一段：驗身分、座位、限流，占名額；本體還沒讀（殼拿到 uploadId 才開始讀） */
export async function uploadReserve(tx: Tx, input: Extract<CoreInput, { route: "upload_reserve" }>) {
  need(tx.exists, "E_ROOM_NOT_FOUND");
  const s = tx.s;
  const m = await memberByDevice(s, input.deviceId);
  need(m, "E_AUTH_FAILED");
  need(!m.banned, "E_BANNED");
  need(s.status !== "closed", "E_ROOM_CLOSED");
  need(s.exec.online, "E_EXECUTOR_OFFLINE");
  need(input.length <= UPLOAD_MAX[input.uploadKind], "E_TOO_LARGE");
  if (input.uploadKind === "card") {
    need(s.settings.allowCustomCards, "E_FORBIDDEN");
    need(!m.cardId && (s.status === "lobby" || s.status === "playing"), "E_STATE_CONFLICT");
  } else need(m.cardId, "E_SEAT_REQUIRED");
  const live = tx.ctx.live;
  const all = Object.values(live.uploads);
  const total = all.reduce((n, u) => n + u.length, 0) + input.length;
  if (all.some((u) => u.memberId === m.id) || all.length >= 3 || total > 16 * MiB) throw new Fail("E_RATE_LIMITED", undefined, 5000);
  rate(tx, `u:${m.id}`, 10, 60 * MIN); // 最後一道檢查：之前任何失敗都不吃每小時的額度
  const uploadId = ulid(tx.now, tx.ctx.rng);
  live.uploads[uploadId] = { memberId: m.id, kind: input.uploadKind, cardId: m.cardId, length: input.length, state: "reserved", at: tx.now, bytes: null };
  tx.http = { status: 200, body: { uploadId } };
}

/** 第二段：本體讀完（或讀到一半斷掉＝null）。驗長度與簽章，合格才掛上並派工 */
export function uploadBody(tx: Tx, input: Extract<CoreInput, { route: "upload_body" }>) {
  const live = tx.ctx.live;
  const u = live.uploads[input.uploadId];
  if (!u || u.state !== "reserved") {
    need(tx.exists && tx.s.exec.online, "E_EXECUTOR_OFFLINE"); // 讀本體期間執行端斷線，名額已收回
    Fail.throw("E_STATE_CONFLICT");
  }
  const drop = (code: ErrorCode): never => {
    delete live.uploads[input.uploadId]; // 失敗只回 HTTP；名額一律收回（live 不跟著交易回滾，要先刪再拋）
    Fail.throw(code);
  };
  if (input.bytes === null) return drop("E_BAD_FRAME");
  const s = tx.s;
  if (s.status === "closed") return drop("E_ROOM_CLOSED");
  if (!s.members.some((m) => m.id === u.memberId && !m.banned)) return drop("E_FORBIDDEN");
  if (!seatedOn(s, u.memberId, u.cardId)) return drop("E_STATE_CONFLICT"); // 卡檔要還沒入座、換圖要還坐在同一張
  if (input.bytes.length !== u.length) return drop("E_BAD_FRAME");
  if (!signatureOk(u.kind, input.bytes)) return drop("E_UNSUPPORTED_FORMAT");
  const jobId = ulid(tx.now, tx.ctx.rng);
  const job: UploadJob =
    u.kind === "card"
      ? { jobId, kind: "card_import", memberId: u.memberId, uploadId: input.uploadId, size: u.length }
      : { jobId, kind: "card_image", memberId: u.memberId, cardId: u.cardId!, uploadId: input.uploadId, size: u.length };
  Object.assign(u, { state: "ready", at: tx.now, bytes: input.bytes });
  tx.w.jobs.push({ job, reply: null });
  tx.toExec(`job.${job.kind}`, job);
  tx.http = { status: 202, body: { uploadId: input.uploadId } };
}

/** 執行端取檔：取一次就丟掉本體，名額留到結果回來；換圖再驗一次上傳者還坐在那張卡上 */
export async function cardFetch(tx: Tx, input: Extract<CoreInput, { route: "card_fetch" }>) {
  need(tx.exists, "E_ROOM_NOT_FOUND");
  const s = tx.s;
  const tokenOk = /^[A-Za-z0-9_-]{43}$/.test(input.executorToken) && s.execHash !== null;
  need(tokenOk && sameHash(await sha256Hex(`${s.code}:${input.executorToken}`), s.execHash!), "E_AUTH_FAILED");
  const u = tx.ctx.live.uploads[input.uploadId];
  need(u?.state === "ready" && u.bytes, "E_DISPATCH_UNKNOWN");
  if (u.kind === "image" && !seatedOn(s, u.memberId, u.cardId)) {
    finishUpload(tx, u.memberId, input.uploadId, { ok: false, code: "E_STATE_CONFLICT" });
    tx.http = { status: 404, body: { code: "E_DISPATCH_UNKNOWN" } };
    return;
  }
  tx.http = { status: 200, body: null, bytes: u.bytes };
  Object.assign(u, { state: "fetched", bytes: null });
}

/** `job.result` 裡的上傳工作（executor.ts 已把工作移出 w.jobs） */
export function uploadResult(tx: Tx, job: Job, p: { ok: boolean; result?: { cardId?: unknown }; error?: { code?: unknown } }) {
  if (!isUploadJob(job)) return;
  let code: ErrorCode | null = p.ok ? null : execCode(p.error?.code);
  let cardId = job.kind === "card_image" ? job.cardId : p.result?.cardId;
  if (!code && job.kind === "card_import") {
    const m = tx.w.members.find((x) => x.id === job.memberId);
    try {
      need(m && typeof cardId === "string", "E_STATE_CONFLICT");
      claim(tx, m, cardId); // 跟自建卡一樣：執行端先送名冊、再回結果，這裡替他入座
    } catch (e) {
      if (!(e instanceof Fail)) throw e;
      code = e.code;
    }
  }
  finishUpload(tx, job.memberId, job.uploadId, code ? { ok: false, code } : { ok: true, cardId: cardId as string });
}

/** 執行端離線、被取代或房間關閉：待處理的全部判失敗（含 DO 逐出後只剩 w.jobs 的上傳工作） */
export function failPending(tx: Tx, code: ErrorCode) {
  const live = tx.ctx.live;
  for (const [reqId, r] of Object.entries(live.cardReqs)) {
    tx.err(r.conn, code, { id: r.id });
    delete live.cardReqs[reqId];
  }
  for (const { job } of tx.s.jobs) if (isUploadJob(job)) finishUpload(tx, job.memberId, job.uploadId, { ok: false, code });
  for (const [id, u] of Object.entries(live.uploads)) finishUpload(tx, u.memberId, id, { ok: false, code });
}

/** 每次輸入的結尾：執行端不在線就清、逾時、讀卡與生圖的人換了座位就作廢（存卡已轉出，照執行端結果回） */
export function settleCards(tx: Tx) {
  const live = tx.ctx.live;
  if (!tx.exists || tx.deleteRoom) return void Object.assign(live, { uploads: {}, cardReqs: {} });
  const s = tx.s;
  if (s.status === "closed" || !s.exec.online) return failPending(tx, s.status === "closed" ? "E_ROOM_CLOSED" : "E_EXECUTOR_OFFLINE");
  for (const [reqId, r] of Object.entries(live.cardReqs)) {
    const code = tx.now >= r.at + REQ_TTL[r.kind] ? "E_EXECUTOR_TIMEOUT" : r.kind !== "save" && !seatedOn(s, r.memberId, r.cardId) ? "E_STATE_CONFLICT" : null;
    if (!code) continue;
    tx.err(r.conn, code, { id: r.id });
    delete live.cardReqs[reqId];
  }
  for (const [id, u] of Object.entries(live.uploads)) if (tx.now >= u.at + UPLOAD_TTL) finishUpload(tx, u.memberId, id, { ok: false, code: "E_EXECUTOR_TIMEOUT" });
}

export function cardsAlarm(live: { uploads: Record<string, Upload>; cardReqs: Record<string, CardReq> }): number {
  const at = [...Object.values(live.uploads).map((u) => u.at + UPLOAD_TTL), ...Object.values(live.cardReqs).map((r) => r.at + REQ_TTL[r.kind])];
  return Math.min(Infinity, ...at);
}

function forward(tx: Tx, kind: CardReq["kind"], m: Member, t: string, fields: Record<string, unknown>) {
  const reqId = ulid(tx.now, tx.ctx.rng);
  tx.ctx.live.cardReqs[reqId] = { kind, conn: tx.conn, id: tx.reqId, memberId: m.id, cardId: m.cardId!, at: tx.now };
  tx.toExec(t, { reqId, memberId: m.id, cardId: m.cardId, ...fields });
  return "later" as const;
}
function mine(tx: Tx, m: Member, cardId?: unknown) {
  need(m.cardId, "E_SEAT_REQUIRED");
  need(cardId === undefined || cardId === m.cardId, "E_FORBIDDEN");
  need(tx.s.exec.online, "E_EXECUTOR_OFFLINE");
}
const field = <T>(v: T | null, code: ErrorCode): T => (v === null ? Fail.throw(code) : v);

export const CARD_MEMBER: Record<string, Handler> = {
  "card.mine": (tx, _p, m) => {
    mine(tx, m);
    rate(tx, `cm:${m.id}`, 1, 1000);
    return forward(tx, "mine", m, "card.read", {});
  },
  // 只送改過的欄位，沒送的執行端照磁碟原值；座位以伺服器收到這一刻為準，之後的換座排在它後面
  "card.save": (tx, p, m) => {
    mine(tx, m, p.cardId);
    const f: Record<string, unknown> = {};
    if (p.name !== undefined) f.name = field(cleanName(p.name, 40), "E_NAME_INVALID");
    if (p.avatar !== undefined) f.avatar = field(cleanIcon(p.avatar), "E_TEXT_INVALID");
    for (const k of ["publicMd", "privateMd"]) if (p[k] !== undefined) f[k] = field(cleanLong(p[k], 64000), "E_TEXT_INVALID");
    need(!(p.removeImage && p.imageFromGallery !== undefined), "E_BAD_FRAME");
    if (p.removeImage) f.removeImage = true;
    if (p.imageFromGallery !== undefined) f.imageFromGallery = field(GALLERY_FILE.test(p.imageFromGallery) ? p.imageFromGallery : null, "E_TEXT_INVALID");
    if (p.crop !== undefined) {
      need(p.imageFromGallery !== undefined, "E_BAD_FRAME"); // 裁切框只用在圖庫圖（上傳的圖手機自己裁好）
      f.crop = field(cleanCrop(p.crop), "E_TEXT_INVALID");
    }
    rate(tx, `cs:${m.id}`, 1, 2000);
    return forward(tx, "save", m, "card.save", f);
  },
  // 自己座位那張卡的圖庫：列一頁（before＝上一頁最後一筆）或取單張預覽；兩種分開限流，剛展開就點縮圖不會撞到
  "card.gallery": (tx, p, m) => {
    mine(tx, m);
    for (const k of ["before", "file"]) if (p[k] !== undefined) need(GALLERY_FILE.test(p[k]), "E_TEXT_INVALID");
    need(p.before === undefined || p.file === undefined, "E_BAD_FRAME");
    if (p.file !== undefined) rate(tx, `cgf:${m.id}`, 4, 2000);
    else rate(tx, `cgl:${m.id}`, 3, 2000);
    return forward(tx, "gallery", m, "card.gallery", p.file !== undefined ? { file: p.file } : p.before !== undefined ? { before: p.before } : {});
  },
  // 花的是 GM 主機上設定的 AI 帳號：主機關了開關就不轉；限流每人同時一件、每小時 6 件，全房同時 2 件
  "image.gen": (tx, p, m) => {
    mine(tx, m, p.cardId);
    need(tx.s.exec.imageGen === true, "E_FORBIDDEN");
    const name = field(cleanName(p.name, 40), "E_NAME_INVALID");
    const description = field(cleanLong(p.description, 3000), "E_TEXT_INVALID");
    const extraPrompt = field(cleanLong(p.extraPrompt, 1000), "E_TEXT_INVALID");
    need(p.framing === "full" || p.framing === "half", "E_BAD_FRAME");
    const gens = Object.values(tx.ctx.live.cardReqs).filter((r) => r.kind === "gen");
    if (gens.some((r) => r.memberId === m.id) || gens.length >= 2) throw new Fail("E_RATE_LIMITED", undefined, 5000);
    rate(tx, `g:${m.id}`, 6, 60 * MIN);
    return forward(tx, "gen", m, "image.gen", { name, description, extraPrompt, framing: p.framing });
  },
};

/** 取出等著的請求；不認得（已逾時、已作廢）就不理 */
function take(tx: Tx, reqId: string, kind: CardReq["kind"]) {
  const r = tx.ctx.live.cardReqs[reqId];
  if (r?.kind !== kind) return null;
  delete tx.ctx.live.cardReqs[reqId];
  return r;
}
const failed = (tx: Tx, r: CardReq, code: unknown) => tx.err(r.conn, execCode(code), { id: r.id });

export const CARD_EXECUTOR: Record<string, (tx: Tx, p: any) => void> = {
  "card.read.result": (tx, p) => {
    const r = take(tx, p.reqId, "mine");
    if (!r) return;
    if (!seatedOn(tx.s, r.memberId, r.cardId)) return tx.err(r.conn, "E_STATE_CONFLICT", { id: r.id }); // 回覆前座位已變：不帶內容
    if (!p.ok) return failed(tx, r, p.code);
    const c = p.card;
    const ok =
      shape(c, "name:s avatar:s publicMd:s privateMd:s image:s~ gallery?:i galleryFile?:s~") && c.name.length <= 200 && c.avatar.length <= 64 &&
      (c.image === null || PNG_URL.test(c.image)) && (c.galleryFile == null || GALLERY_FILE.test(c.galleryFile));
    if (!ok) return tx.err(r.conn, "E_INTERNAL", { id: r.id });
    const card = {
      cardId: r.cardId, name: c.name, avatar: c.avatar, publicMd: c.publicMd, privateMd: c.privateMd, image: c.image,
      gallery: c.gallery ?? 0, galleryFile: c.galleryFile ?? null, // 舊執行端沒報：當沒有圖庫
    };
    tx.send(r.conn, "ok", { id: r.id, result: { card } });
  },
  "card.save.result": (tx, p) => {
    const r = take(tx, p.reqId, "save");
    if (!r) return;
    if (!p.ok) return failed(tx, r, p.code);
    tx.send(r.conn, "ok", { id: r.id });
  },
  "card.gallery.result": (tx, p) => {
    const r = take(tx, p.reqId, "gallery");
    if (!r) return;
    if (!seatedOn(tx.s, r.memberId, r.cardId)) return tx.err(r.conn, "E_STATE_CONFLICT", { id: r.id }); // 回覆前座位已變：不帶內容
    if (!p.ok) return failed(tx, r, p.code);
    const items: unknown[] = p.items ?? [];
    const more = p.more ?? 0;
    // 空頁不能說還有下一頁，免得手機一直翻空頁
    const ok = items.length <= 24 && items.every((i) => shape(i, "file:s thumb:s") && GALLERY_FILE.test(i.file) && PNG_URL.test(i.thumb)) && isCount(more) && (items.length > 0 || more === 0);
    if (!ok) return tx.err(r.conn, "E_INTERNAL", { id: r.id });
    tx.send(r.conn, "ok", { id: r.id, result: { items: items.map((i: any) => ({ file: i.file, thumb: i.thumb })), more } });
  },
  "image.gen.result": (tx, p) => {
    const r = take(tx, p.reqId, "gen");
    if (!r) return;
    if (!p.ok) return failed(tx, r, p.code);
    if (!(GALLERY_FILE.test(p.file ?? "") && PNG_URL.test(p.preview ?? ""))) return tx.err(r.conn, "E_INTERNAL", { id: r.id });
    tx.send(r.conn, "ok", { id: r.id, result: { file: p.file, preview: p.preview } });
  },
};
