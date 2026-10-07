// 匯出成 SillyTavern 聊天檔（D3）：一行一則 JSON（.jsonl），第一行是檔頭，照釘版本 06bde939 的
// saveChat（public/script.js）與聊天匯入（src/endpoints/chats.js：檔頭要有 user_name／name／chat_metadata
// 其一）。有損：只帶對話；變數（MVU）、世界書觸發狀態、卡片 storage 都不帶。卡另外提供原檔下載（D3 的
// 「對話與卡」），在 ST 先匯入卡再匯入聊天檔。
import type { ChatEntry } from "../chat/chat-turn";

export interface StChatInput {
  userName: string;
  characterName: string;
  /** 每則要寫進 `mes` 的字（開場白先代換好巨集，ST 看到的才是玩家看到的） */
  texts: string[];
  entries: ChatEntry[];
  createdAt: number;
}

/** ST 的 humanizedDateTime（RossAscends-mods.js）：本地時間 `YYYY-MM-DD@HHhMMmSSsMSms`。 */
export function humanizedDateTime(timestamp: number): string {
  const date = new Date(timestamp);
  const pad = (value: number, length = 2) => String(value).padStart(length, "0");
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}` +
    `@${pad(date.getHours())}h${pad(date.getMinutes())}m${pad(date.getSeconds())}s${pad(date.getMilliseconds(), 3)}ms`
  );
}

export function toStChat(input: StChatInput): string {
  const header = {
    user_name: input.userName,
    character_name: input.characterName,
    create_date: humanizedDateTime(input.createdAt),
    chat_metadata: {},
  };
  const lines = input.entries.map((entry, index) => ({
    name: entry.role === "user" ? input.userName : input.characterName,
    is_user: entry.role === "user",
    is_system: false,
    send_date: new Date(entry.sentAt ?? input.createdAt).toISOString(),
    mes: input.texts[index] ?? entry.text,
    extra: {},
  }));
  return [header, ...lines].map((line) => JSON.stringify(line)).join("\n");
}
