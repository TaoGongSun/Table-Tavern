// 後端資料契約：src-tauri 回傳的結構，畫面與 controller 共用。
export interface AppConfig {
  api_keys: Record<string, string>;
  tier_models: Record<string, string>;
  preferences: Record<string, unknown>;
}
