// 下載頁的功能對照表：網頁版只保留基礎功能，其餘列出來讓玩家知道桌面版多了什麼（計畫 1.3）。
// 每一列是文案鍵；兩欄給 true／false（有／沒有）或另一個文案鍵（兩邊都有但做法不同）。
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
  { feature: "compare_multi", web: false, desktop: true },
  { feature: "compare_oneLine", web: false, desktop: true },
  { feature: "compare_status", web: false, desktop: true },
  { feature: "compare_scenes", web: false, desktop: true },
  { feature: "compare_editor", web: false, desktop: true },
  { feature: "compare_refactor", web: false, desktop: true },
  { feature: "compare_images", web: false, desktop: true },
  { feature: "compare_usage", web: false, desktop: true },
  { feature: "compare_themes", web: false, desktop: true },
  { feature: "compare_sponsor", web: false, desktop: true },
];
