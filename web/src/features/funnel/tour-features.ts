// 導覽頁的特色卡：順序即頁面順序。shot＝有截圖（左右交錯的圖文），icon＝沒截圖的圖示卡（三欄一排）。
import type { MsgKey } from "../../i18n";

export interface TourFeature {
  key: string;
  title: MsgKey;
  body: MsgKey;
  form: "shot" | "icon";
}

export const TOUR_FEATURES: TourFeature[] = [
  { key: "subscription", title: "tour_subscription_title", body: "tour_subscription_body", form: "shot" },
  { key: "oneLine", title: "tour_oneLine_title", body: "tour_oneLine_body", form: "shot" },
  { key: "status", title: "tour_status_title", body: "tour_status_body", form: "shot" },
  { key: "editor", title: "tour_editor_title", body: "tour_editor_body", form: "shot" },
  { key: "scenes", title: "tour_scenes_title", body: "tour_scenes_body", form: "icon" },
  { key: "local", title: "tour_local_title", body: "tour_local_body", form: "icon" },
];

/** 主截圖（範例桌遊玩畫面）的鍵 */
export const TOUR_HERO = "table";
