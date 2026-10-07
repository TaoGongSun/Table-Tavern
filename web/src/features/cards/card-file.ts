// 卡檔解碼（卡片契約 src/shared/contracts/card-view/card-view.md「解析」）：與桌面版
// src-tauri/src/import/card_io.rs 同一套規則——PNG 只讀 tEXt、chara 優先沒有才 ccv3、嚴格 base64；
// 其餘一律當 JSON。錯誤碼沿用桌面版 UiMsg 的 code，兩邊黃金檔才對得上。

export type CardErrorCode = "png_invalid" | "card_png_no_data" | "card_data_invalid" | "card_json_invalid";

export type CardSource = "png:chara" | "png:ccv3" | "json";

export type DecodedCard =
  | { ok: true; source: CardSource; value: unknown }
  | { ok: false; error: CardErrorCode };

const PNG_MAGIC = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

export function isPng(bytes: Uint8Array): boolean {
  return PNG_MAGIC.every((byte, index) => bytes[index] === byte);
}

class CardError extends Error {
  constructor(readonly code: CardErrorCode) {
    super(code);
  }
}

function matchesAscii(bytes: Uint8Array, at: number, text: string): boolean {
  for (let index = 0; index < text.length; index++) {
    if (bytes[at + index] !== text.charCodeAt(index)) return false;
  }
  return true;
}

/** 第一個關鍵字相符的 tEXt chunk 的值（base64 解開後的 bytes）；沒有回 null。 */
function findCardText(bytes: Uint8Array, keyword: string): Uint8Array | null {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let offset = PNG_MAGIC.length;
  while (offset < bytes.length) {
    if (bytes.length - offset < 12) throw new CardError("png_invalid");
    const length = view.getUint32(offset);
    const chunkEnd = offset + 12 + length;
    if (chunkEnd > bytes.length) throw new CardError("png_invalid");
    if (matchesAscii(bytes, offset + 4, "tEXt")) {
      const data = bytes.subarray(offset + 8, offset + 8 + length);
      // 逐位元組比關鍵字（不把整段展開成字串）：不認得的 tEXt 不論多大都直接略過
      if (data[keyword.length] === 0 && matchesAscii(data, 0, keyword)) {
        return decodeBase64Strict(data.subarray(keyword.length + 1));
      }
    }
    offset = chunkEnd;
  }
  return null;
}

const BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/** 照桌面版 decode_base64：長度 4 的倍數、`=` 只能出現在最後一組的尾端、不收空白。 */
export function decodeBase64Strict(input: Uint8Array): Uint8Array {
  if (input.length % 4 !== 0) throw new CardError("card_data_invalid");
  const output = new Uint8Array((input.length / 4) * 3);
  let written = 0;
  for (let start = 0; start < input.length; start += 4) {
    const group = input.subarray(start, start + 4);
    const padding = group.filter((byte) => byte === 61).length;
    if (
      padding > 2 ||
      (padding > 0 &&
        (group.subarray(0, 4 - padding).includes(61) || group.subarray(4 - padding).some((byte) => byte !== 61)))
    ) {
      throw new CardError("card_data_invalid");
    }
    // 桌面版比的是內容（這一組 ≠ 最後一組才擋），照抄
    const last = input.subarray(input.length - 4);
    if (padding > 0 && group.some((byte, index) => byte !== last[index])) throw new CardError("card_data_invalid");
    const values = [0, 0, 0, 0];
    for (let index = 0; index < 4; index++) {
      const byte = group[index];
      if (byte === 61) continue;
      const value = BASE64.indexOf(String.fromCharCode(byte));
      if (value < 0) throw new CardError("card_data_invalid");
      values[index] = value;
    }
    output[written++] = ((values[0] << 2) | (values[1] >> 4)) & 0xff;
    if (padding < 2) output[written++] = ((values[1] << 4) | (values[2] >> 2)) & 0xff;
    if (padding === 0) output[written++] = ((values[2] << 6) | values[3]) & 0xff;
  }
  return output.subarray(0, written);
}

/** JSON 只認合法 UTF-8；BOM 不剝（serde_json 也不收 BOM）。 */
function parseJson(bytes: Uint8Array): unknown {
  try {
    const text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
    return JSON.parse(text) as unknown;
  } catch {
    throw new CardError("card_json_invalid");
  }
}

export function decodeCardFile(bytes: Uint8Array): DecodedCard {
  try {
    if (!isPng(bytes)) return { ok: true, source: "json", value: parseJson(bytes) };
    const chara = findCardText(bytes, "chara");
    if (chara) return { ok: true, source: "png:chara", value: parseJson(chara) };
    const ccv3 = findCardText(bytes, "ccv3");
    if (ccv3) return { ok: true, source: "png:ccv3", value: parseJson(ccv3) };
    return { ok: false, error: "card_png_no_data" };
  } catch (error) {
    if (error instanceof CardError) return { ok: false, error: error.code };
    throw error;
  }
}

export type JsonObject = Record<string, unknown>;

export const isObject = (value: unknown): value is JsonObject =>
  typeof value === "object" && value !== null && !Array.isArray(value);

/** 卡資料：頂層 `data` 是物件就取它（V2／V3），否則頂層本身（V1）。 */
export function cardData(value: unknown): unknown {
  return isObject(value) && isObject(value.data) ? value.data : value;
}
