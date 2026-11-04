// 玩家卡上傳（room-player-cards §3.1、§3.6）：先自己比上限，POST 原始位元組，拿到 202 的 uploadId 後等 vol.card_upload。
// 圖一律先用 canvas 重畫成 PNG（入口只收 PNG），太大就縮到放得進 2 MiB。
import { UPLOAD_MAX } from "../../../server/core/cards";
import type { ErrorCode } from "../../../server/core/types";
import type { UploadResult } from "../room/useRoom";

export type UploadKind = "card" | "image";
export type Upload = (
  kind: UploadKind,
  body: Blob,
) => Promise<UploadResult & { retryAfterMs?: number }>;

export function makeUpload(
  fetchFn: typeof fetch,
  code: string,
  deviceId: string,
  awaitUpload: (id: string) => Promise<UploadResult>,
): Upload {
  return async (kind, body) => {
    // 宣告超過上限的請求伺服器回完 413 就斷線，瀏覽器常只看到連線錯誤：先擋
    if (body.size > UPLOAD_MAX[kind]) return { ok: false, code: "E_TOO_LARGE" };
    let res: Response;
    try {
      res = await fetchFn(`/api/rooms/${code}/cards?kind=${kind}`, {
        method: "POST",
        headers: { Authorization: `Bearer ${deviceId}` },
        body,
      });
    } catch {
      return { ok: false, code: "E_OFFLINE" as ErrorCode };
    }
    const data = await res.json().catch(() => ({}));
    if (res.status !== 202 || typeof data.uploadId !== "string")
      return { ok: false, code: data.code ?? "E_INTERNAL", retryAfterMs: data.retryAfterMs };
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
  try {
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
  } finally {
    bitmap.close(); // 解碼後的點陣圖佔記憶體，用完就放
  }
}

/** 裁切框：原圖比例 0–1（room-phone-gallery §3.5），跟預覽解析度無關 */
export type CropBox = { x: number; y: number; w: number; h: number };

/** 依裁切框切圖、寬最多 maxWidth（同桌面版裁切窗 1024），出 PNG；瀏覽器解不開就回 null */
export async function cropTo(src: Blob, box: CropBox, maxWidth: number): Promise<Blob | null> {
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(src);
  } catch {
    return null;
  }
  const [sx, sy, sw, sh] = [
    box.x * bitmap.width,
    box.y * bitmap.height,
    box.w * bitmap.width,
    box.h * bitmap.height,
  ];
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(Math.min(sw, maxWidth)));
  canvas.height = Math.max(1, Math.round((sh * canvas.width) / sw));
  canvas.getContext("2d")?.drawImage(bitmap, sx, sy, sw, sh, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  return new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
}
