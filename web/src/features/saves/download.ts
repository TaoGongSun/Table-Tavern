// 把存檔、聊天檔存成玩家電腦裡的檔案（瀏覽器下載）。
import { parseWebSave, type WebSave } from "@desktop/shared/contracts/web-save/web-save";
import type { PlayCard } from "../cards/play-card";
import { humanizedDateTime } from "./st-chat";

/** 檔名不能有的字元換成「_」，前後空白去掉。 */
export function safeFileName(name: string): string {
  return name.replace(/[\\/:*?"<>|\u0000-\u001f]/g, "_").trim() || "Table Tavern";
}

/** 檔名用的本地時間：`2026-10-07 2130`。 */
export function fileStamp(timestamp: number): string {
  const date = new Date(timestamp);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}${pad(date.getMinutes())}`;
}

export function downloadText(fileName: string, text: string | Uint8Array<ArrayBuffer>, type: string): void {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  link.style.display = "none";
  document.body.appendChild(link);
  link.click();
  link.remove();
  // 下載開始後再收回網址（有些瀏覽器點下去當下還沒讀完）
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

/** 匯出前先用契約檢查器自己驗一次：過不了（例如變數表超出上限）就不下載，回錯誤細節。 */
export function downloadWebSave(save: WebSave, title: string, now: number = Date.now()): string | null {
  const text = JSON.stringify(save, null, 2);
  const checked = parseWebSave(text);
  if (!checked.ok) return checked.error.kind === "invalid" ? checked.error.detail : checked.error.kind;
  downloadText(`${safeFileName(title)} ${fileStamp(now)}.json`, text, "application/json");
  return null;
}

export function downloadStChat(text: string, title: string, now: number = Date.now()): void {
  downloadText(`${safeFileName(title)} - ${humanizedDateTime(now)}.jsonl`, text, "application/jsonl");
}

/** 原卡檔（D3 的 ST 匯出帶「對話與卡」）：從 PNG 匯入的給原 PNG，其餘給原卡 JSON 外殼。 */
export function cardFile(card: PlayCard): { name: string; body: string | Uint8Array<ArrayBuffer>; type: string } {
  const title = safeFileName(card.text.name);
  if (card.png) return { name: `${title}.png`, body: new Uint8Array(card.png), type: "image/png" };
  return { name: `${title}.json`, body: JSON.stringify(card.shell, null, 2), type: "application/json" };
}

export function downloadCard(card: PlayCard): void {
  const file = cardFile(card);
  downloadText(file.name, file.body, file.type);
}
