// 兩殼共用的上傳收件（room-player-cards §3.1）：先看標頭、跟核心占名額，拿到名額才串流讀本體，讀完交回核心。
// 只用語言內建：殼把平台的請求本體轉成 AsyncIterable<Uint8Array> 傳進來。
import { UPLOAD_MAX } from "./cards";
import type { RoomRunner } from "./runner";

export interface UploadRequest {
  kind: string | null;
  contentLength: string | null;
  origin: string | null;
  selfOrigin: string;
  deviceId: string;
  body: AsyncIterable<Uint8Array> | null;
}
type Reply = { status: number; body: unknown };
const err = (status: number, code: string): Reply => ({ status, body: { code } });

const DRAIN_MAX = 2 * UPLOAD_MAX.card;

/** 被拒時先把沒讀的本體讀掉丟棄再回（宣告 ≤ 16 MiB 才讀）：伺服器不讀就回，瀏覽器與 wrangler dev 常只看到連線中斷 */
export async function receiveUpload(runner: RoomRunner, req: UploadRequest): Promise<Reply> {
  const reply = await receive(runner, req);
  if (/^\d{1,10}$/.test(req.contentLength ?? "") && Number(req.contentLength) <= DRAIN_MAX) {
    try {
      for await (const _ of req.body ?? []) void _;
    } catch {
      // 對方已斷線
    }
  }
  return reply;
}

async function receive(runner: RoomRunner, req: UploadRequest): Promise<Reply> {
  if (req.origin !== null && req.origin !== req.selfOrigin) return err(403, "E_FORBIDDEN");
  const kind = req.kind;
  if (kind !== "card" && kind !== "image") return err(400, "E_BAD_FRAME");
  if (!/^\d{1,10}$/.test(req.contentLength ?? "")) return err(411, "E_LENGTH_REQUIRED");
  const length = Number(req.contentLength);
  if (length > UPLOAD_MAX[kind]) return err(413, "E_TOO_LARGE");
  const reserved = await runner.run({
    kind: "http",
    route: "upload_reserve",
    deviceId: req.deviceId,
    uploadKind: kind,
    length,
  });
  if (reserved?.http?.status !== 200) return reserved?.http ?? err(500, "E_INTERNAL");
  const uploadId = (reserved.http.body as { uploadId: string }).uploadId;
  const buf = new Uint8Array(length);
  let n = 0;
  let over = false;
  try {
    for await (const chunk of req.body ?? []) {
      if (n + chunk.length > length) {
        over = true;
        break;
      }
      buf.set(chunk, n);
      n += chunk.length;
    }
  } catch {
    n = -1; // 讀到一半斷掉
  }
  const bytes = over || n < 0 ? null : buf.subarray(0, n);
  const out = await runner.run({ kind: "http", route: "upload_body", uploadId, bytes });
  if (over) return err(413, "E_TOO_LARGE");
  return out?.http ?? err(500, "E_INTERNAL");
}
