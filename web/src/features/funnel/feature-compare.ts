// 下載頁的功能對照表：只留不屬於某一種桌的列。各桌專屬的差異寫在該扇門的面板（網頁版有或沒有）。
import type { MsgKey } from "../../i18n";

export interface CompareRow {
  feature: MsgKey;
  web: boolean | MsgKey;
  desktop: boolean | MsgKey;
}

export const FEATURE_COMPARE: CompareRow[] = [
  { feature: "compare_card", web: true, desktop: true },
  { feature: "compare_worldInfo", web: true, desktop: true },
  { feature: "compare_interface", web: true, desktop: true },
  { feature: "compare_saves", web: "compare_saves_web", desktop: "compare_saves_desktop" },
  { feature: "compare_ai", web: "compare_ai_web", desktop: "compare_ai_desktop" },
  { feature: "compare_update", web: "compare_update_web", desktop: "compare_update_desktop" },
  { feature: "compare_scenes", web: false, desktop: true },
  { feature: "compare_editor", web: false, desktop: true },
  { feature: "compare_refactor", web: false, desktop: true },
  { feature: "compare_images", web: false, desktop: true },
  { feature: "compare_usage", web: false, desktop: true },
  { feature: "compare_themes", web: false, desktop: true },
  { feature: "compare_sponsor", web: false, desktop: true },
];
