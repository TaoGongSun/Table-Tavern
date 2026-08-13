// 後端資料契約：src-tauri 回傳的結構，畫面與 controller 共用。

/** list_worlds 的一列：桌 id 與顯示名 */
export interface WorldMeta {
  id: string;
  name: string;
}

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

// 狀態樹節點：葉子是值，分支是子節點（對應後端 StateNode 的 untagged 序列化）
export type StateNode = string | { [key: string]: StateNode };

// 分岔幕的顯示身分：base＝玩家看到的幕號（0 起算），version＝同編號的第幾條，
// parent＝上一幕的內部場號（退回前幕靠它，分岔之後「場號 −1」不再成立）
export interface SceneLabel {
  base: number;
  version: number;
  parent: number | null;
  // 分岔複製來的幕：開頭那則是真實對話而非前情提要，換幕的兩條補救路都不適用
  forked?: boolean;
}

export interface WorldState {
  id: string;
  name: string;
  player_card_id: string | null;
  model_bindings: Record<string, string>;
  current_scene: number;
  catchup_summaries: Record<string, string>;
  // 換幕順手取的幕名：key 是內部場號字串（0 起算），對應後端 WorldState.scene_titles
  scene_titles: Record<string, string>;
  // 分岔後內部場號與顯示編號脫鉤：沒進這張表的幕＝原線，顯示編號就是內部場號
  scene_labels: Record<string, SceneLabel>;
  state: {
    table: Record<string, string>;
    tree: Record<string, StateNode>;
    // 全量桌的跳動警示：路徑（點分）→ 顯示標記（"+40"／"-80"），增量桌一律是空物件
    jumps?: Record<string, string>;
  };
}

// 分支指認清單：auto＝後端同名自動比對出來的結果，還沒真的存進 state.json
export interface BranchBinding {
  path: string[];
  characterId: string;
  characterName: string;
  auto: boolean;
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
