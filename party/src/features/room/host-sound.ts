// 房主提示音開關（room-status-strip §2）：預設開，存不了就只在這次有效。toast 本身沒有開關。
import { useSyncExternalStore } from "react";

const KEY = "tt-party:host-sound";

function read(): boolean {
  try {
    return globalThis.localStorage?.getItem(KEY) !== "0";
  } catch {
    return true;
  }
}

let on = read();
const subs = new Set<() => void>();

export function hostSoundOn(): boolean {
  return on;
}

export function setHostSound(next: boolean): void {
  on = next;
  try {
    globalThis.localStorage?.setItem(KEY, next ? "1" : "0");
  } catch {
    // 存不了就算了
  }
  for (const sub of subs) sub();
}

function subscribe(sub: () => void): () => void {
  subs.add(sub);
  return () => subs.delete(sub);
}

export function useHostSound(): boolean {
  return useSyncExternalStore(subscribe, hostSoundOn, () => true);
}
