// 後端資料契約：src-tauri 回傳的結構，畫面與 controller 共用。

/** list_worlds 的一列。id 是目錄名；解不開的桌仍列出，並標唯讀或需要修復。 */
export interface WorldMeta {
  id: string;
  name: string;
  read_only: boolean;
  needs_repair: boolean;
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
  /** 有正文但被供應商內容過濾或長度上限中途中斷 */
  truncated?: boolean;
  /** 這則系統事件的全文只給 GM 看（角色私設、非公開人物全文）；玩家面的東西都不該拿到它 */
  gm_only?: boolean;
  /** 固定標頭代碼（後端 data/scene/marker.rs 的 EventMarker）；text 只存本文，標頭顯示時才照語系組 */
  marker?: EventMarker;
  /** 開場白（post_opening 寫的那則）；舊紀錄沒有這欄 */
  opening?: boolean;
  /** 穩定 ID（後端落檔時配發）；舊事件沒有，第一次被卡片寫入時補上 */
  id?: string;
  /** 這樓完整的 MVU 變數表（卡片變數模式）；沒有＝這樓尚無表 */
  message_vars?: Record<string, unknown>;
  /** 這樓表的版本 token；表每次變更或復原帶回時換新 */
  vars_rev?: string;
  /** 寫入這張表當下那一幕的 epoch */
  vars_epoch?: string;
  /** GM 回合落檔的冪等鍵 */
  turn_key?: { turn_id: string; part: string };
  /** 寫下這則的玩家動作（送出、旁白、推進、點名各一個）；換幕容量預測靠它切段，舊事件沒有 */
  action_id?: string;
}

/** `append_player_event` 的回傳：落檔的那則，與它在逐字稿檔裡的起始位元組（收回時的收據） */
export interface PlayerAppend {
  event: TranscriptEvent;
  offset: number;
}

// 逐字稿事件標頭代碼：與 Rust `EventMarker` 同形（type 為判別欄）。認不得的 type 走 UnknownMarker，
// 顯示前一律經 features/play/event-text.ts 的 parseMarker 做執行期形狀檢查。
export type KnownMarker =
  | { type: "scene_summary" }
  | { type: "card_arrival"; name: string }
  | { type: "person_arrival"; title: string }
  | { type: "card_private"; name: string }
  | { type: "state_update" }
  | { type: "gm_call"; name: string };
export type UnknownMarker = { type: string };
export type EventMarker = KnownMarker | UnknownMarker;
