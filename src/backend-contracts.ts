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
