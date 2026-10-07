// 玩家名記在這個瀏覽器（只是方便，讀寫失敗就用預設值）。
const KEY = "tt-web:user-name";
/** 名字只收單行、最多 60 字 */
const MAX = 60;

export function cleanUserName(value: string): string {
  return value.replace(/[\r\n]+/g, " ").trim().slice(0, MAX);
}

export function loadUserName(fallback: string): string {
  try {
    return cleanUserName(localStorage.getItem(KEY) ?? "") || fallback;
  } catch {
    return fallback;
  }
}

export function saveUserName(value: string): void {
  try {
    localStorage.setItem(KEY, cleanUserName(value));
  } catch {
    // 瀏覽器不讓存就算了
  }
}
