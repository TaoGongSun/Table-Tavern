// VN 音訊的純規則：音量、要播哪首、這一格帶上哪些音效。不碰 DOM。
import type { FlatCell } from "../vn-cells";
import { isPlayable } from "../vn-cells";

export interface AudioFile {
  kind: string;
  key: string;
  path: string;
}

/** 0–100 的整數。不是數字就用 fallback（沒存過偏好）。 */
export function prefVolume(value: unknown, fallback: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return clampVolume(fallback);
  return clampVolume(value);
}

export function clampVolume(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, Math.round(value)));
}

/** 靜音時音量為 0，滑桿的數字留著。回傳 0–1，給 Audio.volume。 */
export function effectiveVolume(muted: boolean, volume: number): number {
  if (muted) return 0;
  return clampVolume(volume) / 100;
}

/** 兩聲音效起播至少隔這麼久。同時已有兩聲在響，或隔得更近，這聲丟掉。 */
export const SFX_START_GAP_MS = 250;

export function canStartSfx(liveCount: number, lastStartedAt: number | null, now: number): boolean {
  if (liveCount >= 2) return false;
  if (lastStartedAt !== null && now - lastStartedAt < SFX_START_GAP_MS) return false;
  return true;
}

/**
 * null＝這幕還沒寫過配樂，播主題曲。
 * 空陣列＝`[bgm: off]`，停。
 * 否則情緒小寫用 `+` 接起來，跟後端檔名同一套。
 */
export function desiredBgmKey(moods: string[] | null): string | null {
  if (moods === null) return "theme";
  if (moods.length === 0) return null;
  return moods.map((mood) => mood.toLowerCase()).join("+");
}

export function filePath(files: AudioFile[], kind: string, key: string): string | null {
  return files.find((file) => file.kind === kind && file.key === key)?.path ?? null;
}

/**
 * 目前這格連同它後面、下一個可停格之前的正式音效。
 * 暫定格先不播，等落成正式格再播一次。
 */
export function reachedSfx(cells: FlatCell[], flatIndex: number, lastEventKey: string | null): FlatCell[] {
  if (flatIndex < 0 || flatIndex >= cells.length) return [];
  let start = 0;
  for (let index = flatIndex - 1; index >= 0; index -= 1) {
    if (isPlayable(cells[index], lastEventKey)) {
      start = index + 1;
      break;
    }
  }
  let end = cells.length;
  for (let index = flatIndex + 1; index < cells.length; index += 1) {
    if (isPlayable(cells[index], lastEventKey)) {
      end = index;
      break;
    }
  }
  const found: FlatCell[] = [];
  for (let index = start; index < end; index += 1) {
    const cell = cells[index];
    if (cell.kind !== "pending" && cell.cell.type === "sfx") found.push(cell);
  }
  return found;
}
