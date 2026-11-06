// 下載頁以大廳四種開桌方式為主軸：四扇門的 id 對上網址 `#download/<id>`，桌名用大廳既有文案鍵。
// 截圖鍵還沒有圖時頁面不畫（見 tour-images）；subscription／editor／roomHost／roomGuest 的舊圖不再引用。
import type { MsgKey } from "../../i18n";
import type { DownloadTable } from "./download-route";

export interface DoorShot {
  key: string;
  source: "tour" | "guide";
  /** 說明書的手機直式圖 */
  phone?: boolean;
}

export interface DownloadDoor {
  id: DownloadTable;
  /** 大廳同一顆按鈕的名字 */
  name: MsgKey;
  fit: MsgKey;
  /** 網頁版也有這種桌 */
  onWeb: boolean;
  lead: MsgKey;
  points: readonly MsgKey[];
  shots: readonly DoorShot[];
  cardUi?: { title: MsgKey; body: MsgKey; shot: DoorShot };
  /** 房間面板底部連到玩法說明 */
  more?: MsgKey;
}

export const DOWNLOAD_DOORS: readonly DownloadDoor[] = [
  {
    id: "new",
    name: "newTable",
    fit: "door_new_fit",
    onWeb: true,
    lead: "panel_new_lead",
    points: ["panel_new_1", "panel_new_2", "panel_new_3"],
    shots: [
      { key: "table", source: "tour" },
      { key: "status", source: "tour" },
    ],
    cardUi: {
      title: "panel_cardUi_title",
      body: "panel_cardUi_body",
      shot: { key: "cardUi", source: "tour" },
    },
  },
  {
    id: "oneLine",
    name: "genTableBtn",
    fit: "door_oneLine_fit",
    onWeb: false,
    lead: "panel_oneLine_lead",
    points: ["panel_oneLine_1", "panel_oneLine_2", "panel_oneLine_3"],
    shots: [
      { key: "oneLine", source: "tour" },
      { key: "oneLineOutline", source: "tour" },
    ],
  },
  {
    id: "room",
    name: "roomOpen",
    fit: "door_room_fit",
    onWeb: false,
    lead: "panel_room_lead",
    points: ["panel_room_1", "panel_room_2", "panel_room_3"],
    shots: [
      { key: "room-04", source: "guide" },
      { key: "room-15", source: "guide", phone: true },
    ],
    more: "panel_room_more",
  },
  {
    id: "vn",
    name: "vnEntryTitle",
    fit: "door_vn_fit",
    onWeb: false,
    lead: "panel_vn_lead",
    points: ["panel_vn_1", "panel_vn_2", "panel_vn_3"],
    shots: [
      { key: "vnPlay", source: "tour" },
      { key: "vnWizard", source: "tour" },
    ],
  },
];

/** 每種桌都有的圖示卡，順序即頁面順序。不再掛截圖。 */
export const SHARED_FEATURES: readonly { key: string; title: MsgKey; body: MsgKey }[] = [
  { key: "subscription", title: "tour_subscription_title", body: "tour_subscription_body" },
  { key: "local", title: "tour_local_title", body: "tour_local_body" },
  { key: "scenes", title: "tour_scenes_title", body: "tour_scenes_body" },
  { key: "editor", title: "tour_editor_title", body: "tour_editor_body" },
];

/** 主視覺：大廳四格 */
export const TOUR_HERO = "lobby";
