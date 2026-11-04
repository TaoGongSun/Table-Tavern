// 玩家卡上傳（room-player-cards §3.1、§3.6）：先自己比上限，POST 原始位元組，拿到 202 的 uploadId 後等 vol.card_upload。
// 圖一律先用 canvas 重畫成 PNG（入口只收 PNG），太大就縮到放得進 2 MiB。
import { UPLOAD_MAX } from "../../../server/core/cards";
import type { ErrorCode } from "../../../server/core/types";
import type { UploadResult } from "../room/useRoom";

export type UploadKind = "card" | "image";
export type Upload = (kind: UploadKind, body: Blob) => Promise<UploadResult & { retryAfterMs?: number }>;

export function makeUpload(fetchFn: typeof fetch, code: string, deviceId: string, awaitUpload: (id: string) => Promise<UploadResult>): Upload {
  return async (kind, body) => {
    // 宣告超過上限的請求伺服器回完 413 就斷線，瀏覽器常只看到連線錯誤：先擋
    if (body.size > UPLOAD_MAX[kind]) return { ok: false, code: "E_TOO_LARGE" };
    let res: Response;
    try {
      res = await fetchFn(`/api/rooms/${code}/cards?kind=${kind}`, { method: "POST", headers: { Authorization: `Bearer ${deviceId}` }, body });
    } catch {
      return { ok: false, code: "E_OFFLINE" as ErrorCode };
    }
    const data = await res.json().catch(() => ({}));
    if (res.status !== 202 || typeof data.uploadId !== "string") return { ok: false, code: data.code ?? "E_INTERNAL", retryAfterMs: data.retryAfterMs };
    return awaitUpload(data.uploadId);
  };
}

const MAX_SIDE = 2048; // 角色圖用不到更大；桌面版存圖上限 8192 的 PNG 遠超過 2 MiB 上傳上限

/** JPEG／WebP／PNG → PNG；長邊 ≤ 2048，還是超過 2 MiB 就每次縮 0.7 倍再試 */
export async function toPng(file: Blob): Promise<Blob | null> {
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    return null;
  }
  let scale = Math.min(1, MAX_SIDE / Math.max(bitmap.width, bitmap.height));
  for (let i = 0; i < 6; i++, scale *= 0.7) {
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(bitmap.width * scale));
    canvas.height = Math.max(1, Math.round(bitmap.height * scale));
    canvas.getContext("2d")?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    const png = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
    if (!png) return null;
    if (png.size <= UPLOAD_MAX.image) return png;
  }
  return null;
}
