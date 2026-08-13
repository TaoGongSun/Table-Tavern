// 後端資料契約：src-tauri 回傳的結構，畫面與 controller 共用。
export interface AppConfig {
  api_keys: Record<string, string>;
  tier_models: Record<string, string>;
  preferences: Record<string, unknown>;
}

export type Visibility =
  | { type: "gm" }
  | { type: "public" }
  | { type: "characters"; characters: string[] };

export interface WorldbookEntry {
  uid: number;
  title: string;
  keys: string[];
  content: string;
  constant: boolean;
  order: number;
  disabled: boolean;
  locked: boolean;
  visibility: Visibility;
}

// 角色發言 speaker_id 是角色 id；GM 旁白／系統訊息與玩家發言 speaker_id 是空字串，
// speaker_name 是當下顯示名快照——改名後舊事件不動（2026-07-27 拍板），顯示一律讀這欄
export interface TranscriptEvent {
  ts: string;
  speaker_id: string;
  speaker_name: string;
  kind: "dialogue" | "narration" | "player" | "system";
  text: string;
  // 剝殼前的模型原文（狀態區塊與點名行都還在）；沒剝到東西就沒這欄
  raw?: string;
  state?: {
    table: Record<string, string>;
    tree?: Record<string, unknown>;
    notes?: string[];
  };
}
