// 後端資料契約：src-tauri 回傳的結構，畫面與 controller 共用。

/** list_worlds 的一列。id 是目錄名；解不開的桌仍列出，並標唯讀或需要修復。 */
/** 桌型：開桌時定死。chat＝聊天桌、vn＝視覺小說桌（後端缺省 chat） */
export type TableKind = "chat" | "vn";

export interface WorldMeta {
  id: string;
  name: string;
  read_only: boolean;
  needs_repair: boolean;
  table_kind: TableKind;
  /** VN 作品包匯入的重播桌 */
  replay: boolean;
  /** VN 精靈沒走完停在哪一步：大廳標「設定未完成」、點進去回精靈 */
  setup?: VnSetupStage | null;
}

export type VnSetupStage = "story" | "art";

/** VN 精靈 ① 的偏好（vn.json `preferences`）；題材是介面語言文字 */
export interface VnPreferences {
  genres: string[];
  mood: "light" | "warm" | "dark" | null;
  length: "short" | "medium" | "long" | null;
  branching: Bible["branching"] | null;
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
  /** 分岔幕複製的來源幕（劇情祖先鏈）；換幕來的幕省略 */
  source?: number;
  /** 分岔複製了來源幕前幾則；`cut`＝最後一則被複製事件當時的 id（包 6 源頭對應） */
  keep?: number;
  cut?: string;
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
  table_kind: TableKind;
  state: {
    table: Record<string, string>;
    tree: Record<string, StateNode>;
    // 全量桌的跳動警示：路徑（點分）→ 顯示標記（"+40"／"-80"），增量桌一律是空物件
    jumps?: Record<string, string>;
  };
  // 數字欄的顯示寫法（點分路徑→模板，`{}` 是數字的位置，例：「第 {} 天」）：樹上存數字，畫面照模板套回原寫法
  // 顯示前綴（點分路徑→「👤 」這類開頭符號，路徑可含 *）：樹上存不帶符號的值，畫面接回原寫法
  mechanism?: {
    display_formats?: Record<string, string>;
    mirrors?: Record<string, string>;
    display_prefixes?: Record<string, string>;
    // 集合形狀；present_scoped＝在場名單集合（狀態列只列 present 裡的人）
    collections?: Record<string, { present_scoped?: boolean }>;
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
  /** GM 回合落檔的冪等鍵；角色回覆是 `part: "character"`（append_transcript 的 characterTurn，世界書落地認成敗用） */
  turn_key?: { turn_id: string; part: string };
  /** 寫下這則的玩家動作（送出、旁白、推進、點名各一個）；換幕容量預測靠它切段，舊事件沒有 */
  action_id?: string;
}

// VN 劇本格（Rust `chat_turn::VnCell`，以 type 標籤序列化）：`read_vn_cells` 逐則回、`parse_vn_text` 串流回
export type VnCell =
  | { type: "narration"; text: string }
  | {
      type: "dialogue";
      /** 對上名單的角色 id（玩家是 `__PLAYER__`）；null＝沒有卡的配角 */
      speaker_id: string | null;
      speaker_name: string;
      expression: string;
      variant: string | null;
      thought: boolean;
      directive: "closeup" | "shake" | null;
      text: string;
    }
  | {
      type: "bg";
      location: string;
      time: string | null;
      /** 背景庫鍵的地點部分（後端雜湊） */
      slug: string;
      /** 固定時段 morning／day／dusk／night；正式格已補好沿用值，串流暫定格只有明寫的 */
      period: string | null;
    }
  | { type: "choices"; options: string[] }
  | { type: "chapter" }
  | { type: "ending"; id: string };

export interface VnEventCells {
  event_id: string | null;
  kind: TranscriptEvent["kind"];
  cells: VnCell[];
  /** 格邊旗標提示（Rust `vn_bible::FlagHint`）；沒有變動時省略 */
  hints?: FlagHint[];
  /** 這則事件快照的章號（章末字卡用）；快照沒有章號時省略 */
  chapter?: number;
  /** 換幕摘要（前情提要）：播放器不播、只進 log；不是摘要時省略 */
  recap?: boolean;
  /** 選項點（包 6 §2.2）：`answered`＝玩家選的那句，還沒選＝null；不是選項點時省略 */
  choice?: VnChoiceMark;
}

export interface VnChoiceMark {
  answered: string | null;
}

/** `vn_progress_read`：VN 個人進度（Rust `data::vn_progress::Progress`） */
export interface VnProgress {
  version: number;
  /** 已讀格的內容雜湊 */
  read: string[];
  /** 幕號字串 → 最後讀到的格鍵 */
  positions: Record<string, string>;
  saves: VnSave[];
}

export interface VnSave {
  id: string;
  name: string;
  scene: number;
  event_id: string;
  cell: number;
  chapter: number;
  ts: string;
  /** 該格前 40 字 */
  preview: string;
}

/** `vn_save_load`：讀檔後停在這一格 */
export interface VnLoaded {
  scene: number;
  event_id: string;
  cell: number;
}

/** `vn_choice_branches`：選項點上玩過的一條分支（Rust `vn_branches::ChoiceBranch`） */
export interface VnChoiceBranch {
  option: string;
  /** 可切過去的尾幕 */
  tip: number;
  ended: boolean;
  /** 選了這句的那幕與那則玩家句（重播桌直接跳過去用） */
  scene: number;
  answer_event_id: string | null;
}

/** `vn_endings`：結局一覽與完結畫面的已達成 x／y（Rust `vn_bible::EndingsOverview`） */
export interface VnEndings {
  /** 未達成的 title 是空字串（顯示「???」） */
  endings: { id: string; title: string; reached: boolean }[];
  /** 目前這條分支接受的結局 id */
  current: string | null;
}

export type FlagHint =
  | { kind: "flag"; label: string; mark?: string; value?: string }
  | { kind: "reveal"; title: string };

// VN 劇本聖經（Rust `data::vn`，plans/vn-mode-pkg4.md §2.1）
export interface BibleChapter {
  title: string;
  goal: string;
  beats: string;
  end_condition: string;
  plant: string[];
  payoff: string[];
  anchors: string[];
}

export type BibleReveal = { flag: string } | { chapter: number } | "never";

export interface BibleSecret {
  id: string;
  holder: string | null;
  title: string;
  text: string;
  reveal: BibleReveal;
  hint: string;
}

export interface BibleFlag {
  id: string;
  label: string;
  type: "bool" | "counter" | "route";
  initial: string;
  min?: number;
  max?: number;
  values?: string[];
  trigger: string;
}

export interface Bible {
  logline: string;
  style: string;
  branching: "none" | "light" | "full";
  chapter_max: number;
  chapter_reason: string;
  /** VN 卡作者鎖住章數：章數欄唯讀 */
  chapter_lock: boolean;
  chapters: BibleChapter[];
  secrets: BibleSecret[];
  flags: BibleFlag[];
  endings: { id: string; title: string; when: Record<string, unknown>[] }[];
}

/** `vn_bible_read`：鎖、聖經、旗標現值、已揭露清單、能不能重骰 */
export interface BibleView {
  version: number;
  spoiler_lock: boolean;
  bible: Bible | null;
  flagValues: Record<string, string>;
  revealed: { id: string; chapter: number | null }[];
  played: boolean;
  /** 角色 id → 名字（含卡定主角） */
  names: Record<string, string>;
  /** VN 卡建的桌才有：卡原件（只列前端用到的欄位） */
  card?: { protagonist_fixed: boolean; original_bible: Bible | null } | null;
  /** VN 卡匯入時存了封面 */
  cover: boolean;
  /** 卡片變數模式的桌：匯出作品包不能附設定 */
  vars_mode: boolean;
}

// 初始 VN 卡（Rust `import::vn_card`，plans/vn-mode-pkg9.md）
export interface VnCardRating {
  level: "general" | "teen" | "adult";
  warnings: string[];
}

/** `probe_import` 的 `vn_card`：分級確認框要的摘要；supported＝版本認得 */
export interface VnCardProbe {
  supported: boolean;
  title: string;
  rating: VnCardRating;
  cast: string[];
}

export type VnCardNoticeKind =
  | "clipped"
  | "cast_over"
  | "cast_dropped"
  | "color_reset"
  | "protagonist_missing"
  | "holder_dropped"
  | "visibility_dropped"
  | "extras_dropped"
  | "art_dropped";

/** `import_vn_card`：新 VN 桌與夾斷提示 */
export interface VnCardImport {
  world_id: string;
  notices: { kind: VnCardNoticeKind; target: string }[];
}

/** VN 作品包（Rust `vn_work`，plans/vn-mode-pkg8.md）的提示：夾斷、匯出時略過的圖 */
export interface VnWorkNotice {
  kind: "clipped" | "image_skipped" | "dropped";
  target: string;
}

/** `probe_vn_work`：匯入前確認用 */
export interface VnWorkProbe {
  title: string;
  lang: string;
  chapters: number;
  endings: number;
  settings: boolean;
  /** 卡片變數模式的桌匯出的：只能當作品玩 */
  vars_mode: boolean;
  rating: VnCardRating | null;
}

/** `import_vn_work`：新重播桌與夾斷提示；`export_vn_work` 只回提示 */
export interface VnWorkImport {
  world_id: string;
  notices: VnWorkNotice[];
}

// VN 立繪（Rust `vn_sprites`，plans/vn-mode-pkg7.md §3）
export interface FaceBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface SpriteMeta {
  version: number;
  face: FaceBox;
  /** 臉框是預設框：立繪分頁提示手調 */
  face_fallback: boolean;
  variants: { key: string; label: string; face: FaceBox | null }[];
  default: string;
  revs: Record<string, number>;
  /** 定稿是哪張候選裝的；上傳或舊桌沒有 */
  candidate?: string;
}

/** `vn_sprites_read_all`／`vn_sprite_read`：定稿與已生好的臉層（data URL） */
export interface SpriteView {
  meta: SpriteMeta;
  width: number;
  height: number;
  body: string;
  faces: Record<string, string>;
  /** 變體鍵 → 身體圖（共用組已縮成定稿尺寸）與獨立表情組的臉層 */
  variant_layers: Record<string, { body: string; width: number; height: number; faces: Record<string, string> }>;
  /** VN 卡宣告的額外表情（臉層照 faces 的鍵） */
  expressions: { key: string; label: string }[];
  /** VN 卡宣告的變體：還沒生成的在立繪分頁顯示成空格 */
  declared_variants: { key: string; label: string }[];
  /** 舞台身高比例（名單中有填身高的最高者＝1）；null＝照統一比例 */
  scale: number | null;
}

export interface SpriteState {
  sprite: SpriteView | null;
  candidates: string[];
}

/** `vn-sprite-status` 事件與 `vn_sprites_status` */
export interface SpriteSlotStatus {
  worldId: string;
  characterId: string;
  /** null＝定稿與共用表情組；變體鍵＝該變體的獨立表情組 */
  variant: string | null;
  expression: string;
  state: "queued" | "running" | "done" | "failed";
  reason: string | null;
}

/** 一個背景鍵（`vn_backgrounds_fill` 回這幕用到的，依出現順序） */
export interface Backdrop {
  location: string;
  slug: string;
  period: string;
}

/** `vn_background_read`：period＝這張圖實際的時段（與要的時段不同＝同地點基底，舞台疊色調遮罩） */
export interface BackgroundView {
  src: string;
  period: string;
}

export interface BackgroundStatus {
  worldId: string;
  slug: string;
  period: string;
  state: "queued" | "running" | "done" | "failed";
  reason: string | null;
}

/** 串流中的暫定格：最後一格可能還在長 */
export interface VnText {
  cells: VnCell[];
  tail_tentative: boolean;
}

/** `append_player_event` 的回傳：落檔的那則，與它在逐字稿檔裡的起始位元組（收回時的收據） */
export interface PlayerAppend {
  event: TranscriptEvent;
  offset: number;
}

// 逐字稿事件標頭代碼：與 Rust `EventMarker` 同形（type 為判別欄）。認不得的 type 走 UnknownMarker，
// 顯示前一律經 shared/ui/event-text.ts 的 parseMarker 做執行期形狀檢查。
export type KnownMarker =
  | { type: "scene_summary" }
  | { type: "card_arrival"; name: string }
  | { type: "person_arrival"; title: string }
  | { type: "card_private"; name: string }
  | { type: "state_update" }
  | { type: "gm_call"; name: string }
  | { type: "player_watch"; name: string }
  | { type: "player_arrival"; name: string }
  | { type: "player_to_npc"; name: string }
  | { type: "player_absent"; name: string }
  | { type: "player_delegated"; name: string }
  | { type: "player_returned"; name: string };
export type UnknownMarker = { type: string };
export type EventMarker = KnownMarker | UnknownMarker;
