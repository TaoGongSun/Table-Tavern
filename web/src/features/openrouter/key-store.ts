// 金鑰只留在這個瀏覽器的 localStorage，附登出鈕清掉（D7）。不進網址、存檔、錯誤訊息與日誌。
// 私密視窗或封鎖網站資料時存取會丟錯：讀不到就當沒登入，寫不進就只在本次分頁有效。
const KEY = "tt-web:openrouter-key";

export function loadKey(storage: Storage | null): string | null {
  try {
    const value = storage?.getItem(KEY)?.trim();
    return value ? value : null;
  } catch {
    return null;
  }
}

/** 回傳是否真的存進去了（false＝只在本次分頁有效）。 */
export function saveKey(storage: Storage | null, key: string): boolean {
  try {
    storage?.setItem(KEY, key);
    return storage !== null;
  } catch {
    return false;
  }
}

export function clearKey(storage: Storage | null): void {
  try {
    storage?.removeItem(KEY);
  } catch {
    // 清不掉也不擋登出：記憶體裡的金鑰照樣丟掉
  }
}

/** localStorage 本身可能在取用時就丟錯（Safari 封鎖網站資料）。 */
export function browserStorage(kind: "local" | "session"): Storage | null {
  try {
    return kind === "local" ? window.localStorage : window.sessionStorage;
  } catch {
    return null;
  }
}
