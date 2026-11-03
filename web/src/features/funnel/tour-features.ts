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

/**
 * 簡中頁的多人連線段（排在特色卡前，各配一張簡中截圖）：作者只要求簡中頁改版（2026-11-03），文案固定簡中、不進十語字典。
 */
export const ROOM_TOUR_ZH_CN = {
  title: "怎么和朋友一起玩",
  features: [
    {
      key: "roomHost",
      title: "房主：桌面版开房，当 GM",
      body: "房主在桌面版首页点“开房间”，选角色卡（也可以不选）就开好一张新桌的房间，把邀请链接或二维码发到群里。AI 额度由房主出，电脑替大家跑 GM；房主直接在电脑上一起玩，不用另配手机。",
    },
    {
      key: "roomGuest",
      title: "朋友：手机点链接就加入",
      body: "朋友不用装 App，手机浏览器点开邀请链接、选一个角色就能上桌。每人写下这一轮的行动，到齐后 GM 一起结算，剧情实时流到每个人的手机上。",
    },
  ],
};
