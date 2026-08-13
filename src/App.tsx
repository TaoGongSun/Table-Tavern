import { FormEvent, Fragment, PointerEvent as ReactPointerEvent, useEffect, useMemo, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { confirm, message as showMessage, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { detectLang, Lang, LANGUAGE_OPTIONS, normalizeLang, setLang, t, type MsgKey } from "./i18n";
import { renderStoryMarkdown } from "./story-markdown";
import { buildShellDocument, CardInterface, CardStorage, findShell, sanitizeCardStorage } from "./interface-card";
import { decideImportRoute } from "./import-routing";
import { isCharacterHidden } from "./character-visibility";
import {
  applyCachedCatalogs,
  CATALOG_SOURCES,
  mergeCatalog,
  parseOpenRouterModels,
  tierLabel,
  type ModelCatalogs,
  type ModelOption,
} from "./model-catalog";
import {
  assembleRefactorOutcome,
  buildRefactorPersonPlan,
  defaultRefactorSelection,
  parseRefactorOutcome,
  refactorSummaryCounts,
  REFACTOR_IMPORT_INVALID,
  restoreDropped,
  setPlayerIndex,
  sourceEntryTitle,
  sourceEntryTitles,
  toggleIndex,
  unselectCharacter,
  type RefactorApplySummary,
  type RefactorCharacter,
  type RefactorExpandOutcome,
  type RefactorInterface,
  type RefactorLocalAssembly,
  type RefactorNewEntry,
  type RefactorRewriteOutcome,
  type RefactorOutcome,
  type RefactorPersonExpandOutcome,
  type RefactorPersonQueueItem,
  type RefactorSelection,
  type RefactorSplitGroup,
  type RefactorSurveyOutcome,
} from "./refactor-review";
import { REFACTOR_PARALLEL_LIMIT, runRefactorCalls, withRateLimitRetry } from "./refactor-run";
import { fillShellPlaceholders, fillSkeletonPlaceholders } from "./refactor-shell";
import { explainAiError } from "./ai-error";
import { KOFI_URL } from "./appearance";
import { AppConfig } from "./backend-contracts";
import { CharacterCard, CharacterMeta, PALETTE } from "./card-model";
import { cachedClis, CLI_LABELS, CliInfo, detectClis } from "./cli";
import { CardEditor } from "./views/CardEditor";
import taoIcon from "./assets/tao-icon.png";
import gmBook from "./assets/gm-book.png";
import "./App.css";

// 主題清單：free 兩套隨點隨存；sponsor 五套未解鎖只能試看（關設定視窗即復原）
const FREE_THEMES = ["dark", "light"] as const;
const SPONSOR_THEMES = ["parchment", "herbal", "candlelight", "port", "seamist"] as const;
const THEME_LABEL_KEYS = { dark: "themeDark", light: "themeLight", parchment: "themeParchment", herbal: "themeHerbal", candlelight: "themeCandlelight", port: "themePort", seamist: "themeSeamist" } as const;
// 色票縮圖用色（與 App.css 各主題 surface-0／accent 同步）
const THEME_SWATCH: Record<string, { bg: string; dot: string }> = {
  dark: { bg: "#20242c", dot: "#e58057" },
  light: { bg: "#e8e8e8", dot: "#b85a35" },
  parchment: { bg: "#eee8d5", dot: "#a2470e" },
  herbal: { bg: "#e2eadb", dot: "#3e6b34" },
  candlelight: { bg: "#251e15", dot: "#e0a24e" },
  port: { bg: "#241a20", dot: "#d9899b" },
  seamist: { bg: "#e1e8eb", dot: "#2c6e86" },
};
const ALL_THEMES = [...FREE_THEMES, ...SPONSOR_THEMES] as const;
type ThemeId = (typeof ALL_THEMES)[number];

interface CharacterImport {
  meta: CharacterMeta;
  book: WorldbookImport;
}

interface ImportProbe {
  lorebook_heavy: boolean;
  /** 卡名（世界書卡也有）：匯入後自動名桌拿它當桌名 */
  name?: string | null;
  /** JSON／PNG 成功解析才 true：分辨「格式錯誤」與「解析成功但沒有名字」 */
  parsed: boolean;
  /** character_book.entries 的條目數，沒有這個欄位就是 0 */
  book_entries: number;
  /** 頂層就是世界書本體（V2 獨立書 JSON 自帶 name＋entries）：有 name 也要走世界書 */
  book_shaped: boolean;
  /** 備用開場白數：備了好幾個開局＝這是一座舞台不是一個人（first_mes 每張卡都有，不看） */
  alternate_greetings: number;
}

/** 身分框主按鈕指向世界書的條件。判準只決定哪顆是主按鈕與文案講哪一種，
 *  兩條路玩家都選得到——判錯的代價是多看一眼，不是卡壞掉。 */
function looksLikeWorldbook(probe: ImportProbe): boolean {
  return probe.book_shaped || probe.lorebook_heavy || probe.alternate_greetings > 0;
}

/** 世界書匯入結果：skipped＝內容和現有條目一模一樣、被略過的條數 */
interface WorldbookImport {
  imported: number;
  skipped: number;
}

/** 匯入收據摘要：側欄「復原上次匯入」按鈕靠這份判斷要不要顯示 */
interface ImportReceiptSummary {
  kind: "character" | "worldbook";
  label: string;
  timestamp: string;
  character_id?: string | null;
}

/** 復原上次匯入的結果：kept_entries＝玩家改過內容而保留下來的世界書條目數 */
interface UndoReport {
  removed_character?: string | null;
  /** AI 卡重構等一次套用多張角色卡的路徑：這次 undo 刪掉的角色名字清單 */
  removed_characters: string[];
  removed_entries: number;
  kept_entries: number;
  renamed_back: boolean;
  /** 匯完貼上檯面的那則開場白也被收掉了：前端據此重載逐字稿 */
  removed_opening: boolean;
}

// 角色發言 speaker_id 是角色 id；GM 旁白／系統訊息與玩家發言 speaker_id 是空字串，
// speaker_name 是當下顯示名快照——改名後舊事件不動（2026-07-27 拍板），顯示一律讀這欄
interface TranscriptEvent {
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

// 串流中的旁白尾端會冒出狀態區塊，整則寫完才由後端剝乾淨；
// 這裡先切掉，免得玩家每回合都看到一段圍欄或標籤閃過去
function narrationStreamText(text: string) {
  const marker = text.search(/```|<details|<status|<UpdateVariable/i);
  return marker === -1 ? text : text.slice(0, marker);
}

function StoryText({ text }: { text: string }) {
  const html = useMemo(() => renderStoryMarkdown(text), [text]);
  return (
    <span
      className="text rendered"
      // 卡片內嵌的圖是熱連作者自己的圖床，失效是常態（擋熱連、圖被刪、玩家離線）：
      // 載不到就整張藏掉，故事中間留一個破圖示比沒有更糟。img 的 error 不冒泡，只能用 capture 收
      onErrorCapture={(event) => {
        const target = event.target as HTMLElement;
        if (target.tagName === "IMG") target.classList.add("img-failed");
      }}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

interface WorldMeta {
  id: string;
  name: string;
}

interface GeneratedOutline {
  title: string;
  world: string;
  characters: { name: string; tagline: string }[];
}

interface GenerateOutlineResult {
  parsed: GeneratedOutline | null;
  raw: string;
}

interface GenerateExpandResult {
  worldId: string | null;
  raw: string;
}

interface GenerateCharacterResult {
  parsed: { name: string; tagline: string } | null;
  raw: string;
}

function serializeGeneratedOutline(outline: GeneratedOutline): string {
  const sections = [`## WORLD: ${outline.title.trim()}\n${outline.world.trim()}`];
  for (const character of outline.characters) {
    const name = character.name.trim();
    if (name) sections.push(`## CHARACTER: ${name}\n${character.tagline.trim()}`);
  }
  return sections.join("\n\n");
}

function resizeGeneratedCharacterTagline(target: HTMLTextAreaElement) {
  target.style.height = "auto";
  target.style.height = `${target.scrollHeight}px`;
}

interface WorldState {
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

// 分岔幕的顯示身分：base＝玩家看到的幕號（0 起算），version＝同編號的第幾條，
// parent＝上一幕的內部場號（退回前幕靠它，分岔之後「場號 −1」不再成立）
interface SceneLabel {
  base: number;
  version: number;
  parent: number | null;
  // 分岔複製來的幕：開頭那則是真實對話而非前情提要，換幕的兩條補救路都不適用
  forked?: boolean;
}

// 狀態樹節點：葉子是值，分支是子節點（對應後端 StateNode 的 untagged 序列化）
type StateNode = string | { [key: string]: StateNode };

// 路徑指到的葉子值；中途撞到分支或缺節點都當空字串（面板只讀，取不到就是沒東西可改）
// 殼字串的短指紋（djb2）：card-interface iframe 的 key 用，殼一換 key 就換。
function shellFingerprint(shell: string | null): string {
  if (shell === null) return "empty";
  let hash = 5381;
  for (let i = 0; i < shell.length; i++) hash = ((hash << 5) + hash + shell.charCodeAt(i)) | 0;
  return String(hash >>> 0);
}

// 卡片介面殼在沙盒裡的 localStorage 存這裡（一桌一份）：殼每次重掛都是全新的沙盒，玩家在卡片
// 設定分頁調的主題／字級要靠宿主這側留著再回填。內容是第三方 JS 寫的，讀寫都先過 sanitize。
const CARD_STORAGE_PREFIX = "card-storage:";

function readCardStorage(worldId: string | null): CardStorage {
  if (worldId === null) return {};
  try {
    const raw = window.localStorage.getItem(CARD_STORAGE_PREFIX + worldId);
    return raw === null ? {} : (sanitizeCardStorage(JSON.parse(raw)) ?? {});
  } catch {
    return {};
  }
}

function writeCardStorage(worldId: string | null, entries: unknown): void {
  const clean = worldId === null ? null : sanitizeCardStorage(entries);
  if (clean === null) return;
  try {
    window.localStorage.setItem(CARD_STORAGE_PREFIX + worldId, JSON.stringify(clean));
  } catch {
    // 宿主這側寫不進去（配額滿等）：卡片設定這回合留在沙盒記憶體裡，不影響畫面
  }
}

function treeValueAt(tree: Record<string, StateNode>, path: string[]): string {
  let node: StateNode | undefined = tree[path[0]];
  for (const key of path.slice(1)) {
    if (typeof node !== "object" || node === null) return "";
    node = node[key];
  }
  return typeof node === "string" ? node : "";
}

// 分支指認清單：auto＝後端同名自動比對出來的結果，還沒真的存進 state.json
interface BranchBinding {
  path: string[];
  characterId: string;
  characterName: string;
  auto: boolean;
}

// 值裡的字面 {{user}} 只在顯示時換成玩家名（模型上下文與存檔仍是原文，後端注入前才代換）；
// 大小寫不分、容許中間空白（{{ user }}），其他巨集不動
const USER_MACRO = /\{\{\s*user\s*\}\}/gi;
function displayUserMacro(value: string, playerName: string): string {
  return value.replace(USER_MACRO, playerName);
}

type Visibility =
  | { type: "gm" }
  | { type: "public" }
  | { type: "characters"; characters: string[] };

interface WorldbookEntry {
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

interface WorldbookDraft {
  uid: number | null;
  title: string;
  keys: string;
  content: string;
  constant: boolean;
  enabled: boolean;
  order: number;
  visibility: Visibility["type"];
  characters: string[];
}

// 機制帳本：世界書分頁「哪些條目被本地機制接管／跳過」面板，對應 mechanism.rs 的 Ledger。
type RecordKind = "rejected" | "clamped" | "error" | "absorbed" | "skipped" | "jump";

interface LedgerEntry {
  uid: number;
  title: string;
  kind: RecordKind;
  detail: string;
  sent: boolean;
}

interface Ledger {
  entries: LedgerEntry[];
  rejected: number;
  clamped: number;
  errors: number;
  jumps: number;
}

const EMPTY_LEDGER: Ledger = { entries: [], rejected: 0, clamped: 0, errors: 0, jumps: 0 };

// 主題不跟系統走（2026-07-25 使用者拍板）：config.preferences.theme 寫在 <html data-theme>，預設深色
function resolveTheme(config: AppConfig | null | undefined, sponsorUnlocked: boolean): ThemeId {
  const theme = String(config?.preferences["theme"] ?? "dark");
  if (!ALL_THEMES.includes(theme as ThemeId)) return "dark";
  if (
    (SPONSOR_THEMES as readonly string[]).includes(theme) &&
    !sponsorUnlocked
  ) {
    return "dark";
  }
  return theme as ThemeId;
}

// 檔位預設模型只是設定欄的預填建議（存進 config.json 後由使用者作主），程式邏輯不讀它
const SUGGESTED_TIER_MODELS: Record<string, string> = {
  best: "anthropic/claude-opus-4.8",
  balanced: "anthropic/claude-sonnet-5",
  fast: "google/gemini-3.5-flash",
};

// GM 點到玩家時後端回這個代號（transport.rs 的 PLAYER_SENTINEL），收到就把發言權交回給玩家
const PLAYER_SENTINEL = "__PLAYER__";
// 發言對象是 GM 時 speaker 存這個代號（純前端狀態，不會寫進紀錄）；GM 以旁白回應
const GM_TARGET = "__GM__";
// GM 卡的銅金色：發言對象晶片沿用書皮的 --fac，與角色卡的陣營色區隔
const GM_COLOR = "#8a6a3c";

// 側欄寬度是純 UI 狀態，存瀏覽器端即可，不進 config.json。
// 下限擋在這裡，上限交給 CSS 的 max-width: 50%（視窗縮小時自動夾住）。
const SIDEBAR_WIDTH_KEY = "sidebar_width";
const TABLE_LIST_OPEN_KEY = "table_list_open";
const STATE_BAR_OPEN_KEY = "state_bar_open";
// 這桌向 AI 發過對話請求了沒（每桌一把）。開演之後復原＝把演到一半的角色卡連同後續編輯一起刪掉，
// 所以按鈕要收起來；記在瀏覽器端，重開 app 也不該讓它又冒出來讓人誤按。
const chattedKey = (worldId: string) => `chatted_since_import:${worldId}`;
const SIDEBAR_DEFAULT_WIDTH = 224;
const SIDEBAR_MIN_WIDTH = 176;
const SIDEBAR_KEY_STEP = 16;
const GENRE_KEYS = [
  "genGenreFantasy",
  "genGenreScifi",
  "genGenreUrban",
  "genGenreWuxia",
  "genGenreSchool",
  "genGenreApocalypse",
] as const;

// 模型清單接線：合併規則與 OpenRouter 解析在 model-catalog.ts，這裡只管抓取、落地與訂閱
let catalogStore: ModelCatalogs = {};
const catalogListeners = new Set<() => void>();
let catalogPrefetched = false;

function publishCatalogs(next: ModelCatalogs) {
  catalogStore = next;
  catalogListeners.forEach((listener) => listener());
}

/// 訂閱模組級清單：抓取在背景完成，回來時所有掛著的畫面一起換上新的。
function useModelCatalogs(): ModelCatalogs {
  const [snapshot, setSnapshot] = useState(catalogStore);
  useEffect(() => {
    const listener = () => setSnapshot(catalogStore);
    catalogListeners.add(listener);
    listener(); // 掛上前抓完的那幾支要補回來
    return () => {
      catalogListeners.delete(listener);
    };
  }, []);
  return snapshot;
}

async function fetchCatalog(id: string): Promise<ModelOption[]> {
  if (id === "api") {
    // OpenRouter 公開清單（免 key）；拿不到就退化成純手動輸入
    return parseOpenRouterModels(await (await fetch("https://openrouter.ai/api/v1/models")).json());
  }
  return invoke<ModelOption[]>("list_cli_models", { cli: id });
}

/// 抓一支並落地；抓到空的由 mergeCatalog 留住上次的結果。
/// catalogStore 一律在 await 之後才讀：五家並行預熱，拿抓取前的舊值去算會互相覆蓋。
async function refreshCatalog(id: string): Promise<void> {
  try {
    const fetched = await fetchCatalog(id);
    const merged = mergeCatalog(catalogStore, id, fetched);
    if (merged === catalogStore) return;
    publishCatalogs(merged);
    await invoke("write_model_catalog", { catalog: catalogStore });
  } catch {
    /* 抓不到就沿用快取 */
  }
}

/// 開 app 時跑一次：先把上次存的清單擺上（玩家點進設定即刻有東西可選），
/// 再五家並行重抓，回來一支換一支。
async function prefetchModelCatalogs(): Promise<void> {
  if (catalogPrefetched) return;
  catalogPrefetched = true;
  try {
    const cached = await invoke<ModelCatalogs>("read_model_catalog");
    publishCatalogs(applyCachedCatalogs(catalogStore, cached));
  } catch {
    /* 沒有快取檔就等抓取回來 */
  }
  await Promise.all(CATALOG_SOURCES.map((id) => refreshCatalog(id)));
}

type CliInstallStage = "detect" | "install" | "login" | "verify" | "done" | "error";

interface CliInstallProgress {
  provider: string;
  stage: CliInstallStage;
  detail?: string;
  logPath?: string;
}

function cliInstallStageText(stage: CliInstallStage) {
  switch (stage) {
    case "detect":
      return t("cliInstallStageDetect");
    case "install":
      return t("cliInstallStageInstall");
    case "login":
      return t("cliInstallStageLogin");
    case "verify":
      return t("cliInstallStageVerify");
    case "done":
      return t("cliInstallStageDone");
    case "error":
      return t("cliInstallStageError");
  }
}

// PowerShell 的錯誤代號與安裝腳本自身的訊息固定為英文，不受系統語言影響，
// 可靠地認出「安裝檔被其他程序鎖住」（防毒掃描中、工具還在跑）這類失敗
const FILE_LOCKED_MARKERS = [
  "RemoveFileSystemItemIOError",
  "being used by another process",
  "Failed to install",
];

// 連不上服務商（下載失敗的 PowerShell 錯誤代號）或登入視窗沒走完（我們自己的錯誤字串）
const NETWORK_MARKERS = [
  "InvokeRestMethodCommand",
  "InvokeWebRequestCommand",
  "login window closed or timed out",
  "verification failed",
];

function cliInstallErrorHint(detail: string | undefined) {
  if (!detail) {
    return null;
  }
  if (FILE_LOCKED_MARKERS.some((marker) => detail.includes(marker))) {
    return t("cliInstallHintFileLocked");
  }
  if (NETWORK_MARKERS.some((marker) => detail.includes(marker))) {
    return t("cliInstallHintNetwork");
  }
  return null;
}

const CLI_INSTALL_URLS: Record<string, string> = {
  claude: "claude.ai",
  codex: "chatgpt.com/codex",
  agy: "antigravity.google",
  grok: "x.ai/cli",
};

const CLI_IDS = ["claude", "codex", "agy", "grok"] as const;

function cliConnectedKey(id: string) {
  return `cli_connected:${id}`;
}

// 系統權限預告只在每家 CLI 第一次啟用時彈一次；說明本身在設定頁常駐，事後查得到
function cliNoticeKey(id: string) {
  return `cli_permission_notice:${id}`;
}

const CLI_RISK_KEYS = ["risk1", "risk2", "risk3", "risk4"] as const;

// 換場提醒門檻：粗略以字元數估算紀錄長度，不精算 token。
// 快取上線後換幕不再省額度（摘要與換幕後首輪都全額計價，約等於連跑四輪），
// 提醒的理由改成「紀錄長到模型顧不上前面」，門檻從 8000 提到 30000（2026-08-04 實測拍板）。
const SCENE_LENGTH_HINT_CHARS = 30000;

// 保溫 ping（prompt-cache-optimization 包 7）：快取只活五分鐘，玩家慢慢想的時候先讀一次
// 既有快取把壽命重新計時，代價約為讓它過期重建的十二分之一。連三次（約十二分鐘）都沒等到
// 玩家推進就收手改提示換幕——人真的離開時，長紀錄每次回來都要全額重建，那時短紀錄才便宜。
const KEEPALIVE_TICK_MS = 30 * 1000;
const KEEPALIVE_AFTER_MS = 3.5 * 60 * 1000;
const KEEPALIVE_MAX_PINGS = 3;
// 離開太久的換幕提醒還要紀錄夠長才有意義：短紀錄重建本來就便宜，換幕反而多花一次摘要錢。
// 保溫仍照樣停在三次（那是省錢邏輯），這個門檻只決定要不要出聲提醒。
const SCENE_AWAY_HINT_MIN_CHARS = 8000;

function nowTs() {
  return new Date().toISOString();
}

function openingPreview(text: string) {
  const preview = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .slice(0, 2)
    .join(" ");
  return preview.length > 120 ? `${preview.slice(0, 119)}…` : preview;
}

// 錯誤列：命中分流就顯示人話，原始字串一律保留在小字（玩家與協助者仍看得到真相）
function ErrorNote({ text }: { text: string }) {
  const key = explainAiError(text);
  if (!key) return <p role="alert">{text}</p>;
  return (
    <p role="alert">
      {t(key)}
      <br />
      <small>{text}</small>
    </p>
  );
}

function Onboarding({ config, onSaved }: { config: AppConfig; onSaved: (c: AppConfig) => void }) {
  const [apiKey, setApiKey] = useState("");
  const [message, setMessage] = useState("");
  const transport = config.preferences["transport"] ?? "api";

  if (transport !== "api" || (config.api_keys["openrouter"] ?? "").trim()) return null;

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    const next: AppConfig = {
      ...config,
      api_keys: { ...config.api_keys, openrouter: apiKey.trim() },
    };
    try {
      await invoke("write_config", { config: next });
      onSaved(next);
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <section className="settings onboarding" role="note">
      <form className="settings-form" onSubmit={save}>
        <strong>{t("onboardTitle")}</strong>
        <p>{t("onboardIntro")}</p>
        <ol>
          <li>
            {t("onboardStep1")}
            <button type="button" onClick={() => void openUrl("https://openrouter.ai/")}>
              {t("onboardStep1Btn")}
            </button>
          </li>
          <li>{t("onboardStep2")}</li>
          <li>
            {t("onboardStep3")}
            <button
              type="button"
              onClick={() => void openUrl("https://openrouter.ai/settings/keys")}
            >
              {t("onboardStep3Btn")}
            </button>
          </li>
        </ol>
        <p>{t("onboardCost")}</p>
        <div className="row">
          <input
            type="password"
            aria-label={t("apiKeyLabel")}
            value={apiKey}
            onChange={(event) => setApiKey(event.currentTarget.value)}
            placeholder="sk-or-..."
          />
          <button type="submit">{t("onboardSaveBtn")}</button>
        </div>
        {message && <span role="alert">{message}</span>}
        <small>{t("onboardCliHint")}</small>
      </form>
    </section>
  );
}

function Settings({
  config,
  onSaved,
  onDirty,
}: {
  config: AppConfig;
  onSaved: (c: AppConfig) => void;
  onDirty: (count: number) => void;
}) {
  const [apiKey, setApiKey] = useState(config.api_keys["openrouter"] ?? "");
  const [tierModels, setTierModels] = useState<Record<string, string>>({
    ...SUGGESTED_TIER_MODELS,
    ...config.tier_models,
  });
  const [baseUrl, setBaseUrl] = useState(String(config.preferences["base_url"] ?? ""));
  const [imageModel, setImageModel] = useState(String(config.preferences["image_model"] ?? ""));
  const [claudeCompatBaseUrl, setClaudeCompatBaseUrl] = useState(
    String(config.preferences["claude_base_url"] ?? ""),
  );
  const [claudeCompatKey, setClaudeCompatKey] = useState(config.api_keys["claude_compat"] ?? "");
  const [gmTier, setGmTier] = useState(String(config.preferences["gm_tier"] ?? "best"));
  const [maxRound, setMaxRound] = useState(String(config.preferences["max_round_speakers"] ?? 3));
  const [transport, setTransport] = useState(String(config.preferences["transport"] ?? "api"));
  const [permissionNotice, setPermissionNotice] = useState("");
  const [riskAccepted, setRiskAccepted] = useState(config.preferences["cli_risk_accepted"] === true);
  const [clis, setClis] = useState<CliInfo[] | null>(cachedClis());
  const catalogs = useModelCatalogs();
  const [customTiers, setCustomTiers] = useState<Record<string, boolean>>({});
  const [message, setMessage] = useState("");
  const [installingCli, setInstallingCli] = useState<string | null>(null);
  const [installProgress, setInstallProgress] = useState<Record<string, CliInstallProgress>>({});
  const cliPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  function stopCliPolling() {
    if (cliPollRef.current !== null) {
      clearInterval(cliPollRef.current);
      cliPollRef.current = null;
    }
  }

  useEffect(() => {
    detectClis().then(setClis).catch(() => setClis([]));
    // 模型清單不在這裡抓：開 app 就預熱好了（見 prefetchModelCatalogs），
    // 這裡只透過 useModelCatalogs 訂閱結果
    return stopCliPolling;
  }, []);

  // 監聽器只掛一次；config／onSaved 走 ref 取最新值，避免安裝中重掛掉事件
  const configRef = useRef(config);
  configRef.current = config;
  const onSavedRef = useRef(onSaved);
  onSavedRef.current = onSaved;
  useEffect(() => {
    let disposed = false;
    let stopListening: (() => void) | undefined;
    void listen<CliInstallProgress>("cli-install-progress", (event) => {
      setInstallProgress((previous) => ({
        ...previous,
        [event.payload.provider]: event.payload,
      }));
      if (event.payload.stage === "done" || event.payload.stage === "error") {
        setInstallingCli((current) => (current === event.payload.provider ? null : current));
        const base = configRef.current;
        const next = {
          ...base,
          preferences: {
            ...base.preferences,
            [cliConnectedKey(event.payload.provider)]: event.payload.stage === "done",
          },
        };
        void invoke("write_config", { config: next }).then(() => onSavedRef.current(next)).catch(() => {});
        if (event.payload.stage === "done") {
          void detectClis(true).then(setClis).catch(() => {});
          // 剛登入完才拿得到完整清單（未登入時 grok 只回得出一個預設模型），
          // 預熱那次抓的是登入前的結果，這裡補抓一次
          void refreshCatalog(event.payload.provider);
        }
      }
    }).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        stopListening = unlisten;
      }
    });
    return () => {
      disposed = true;
      stopListening?.();
    };
  }, []);

  async function installCli(provider: string) {
    const repeat = installingCli === provider;
    if (!repeat) {
      setInstallingCli(provider);
      setInstallProgress((previous) => {
        const next = { ...previous };
        delete next[provider];
        return next;
      });
    }
    const params = {
      provider: CLI_LABELS[provider],
      url: CLI_INSTALL_URLS[provider],
    };
    try {
      await invoke("install_cli", {
        provider,
        messages: {
          start: t("cliInstallStart", params),
          loginHint: t("cliInstallLoginHint", params),
          success: t("cliInstallSuccess", params),
          fail: t("cliInstallFail", params),
        },
      });
    } catch (reason) {
      const error = String(reason);
      const cooldown = error.match(/^login-cooldown:(\d+)$/);
      setMessage(cooldown ? t("cliLoginCooldown", { secs: cooldown[1] }) : error);
      if (!repeat) setInstallingCli(null);
      return;
    }

    let elapsed = 0;
    stopCliPolling();
    cliPollRef.current = setInterval(() => {
      elapsed += 3_000;
      // 輪詢的目的就是等 CLI 裝好出現，必須繞過快取重探
      void detectClis(true).then(setClis).catch(() => {});
      // 安裝流程跑在獨立終端機／背景工作裡，只能靠 cli_verified 印記知道登入驗證過了沒
      invoke<boolean>("cli_verified", { provider })
        .then((verified) => {
          if (!verified) {
            if (elapsed >= 600_000) {
              stopCliPolling();
              setInstallingCli(null);
            }
            return;
          }
          stopCliPolling();
          setInstallingCli(null);
          const base = configRef.current;
          const next = {
            ...base,
            preferences: { ...base.preferences, [cliConnectedKey(provider)]: true },
          };
          void invoke("write_config", { config: next })
            .then(() => onSavedRef.current(next))
            .catch(() => {});
        })
        .catch(() => {
          if (elapsed >= 600_000) {
            stopCliPolling();
            setInstallingCli(null);
          }
        });
    }, 3_000);
  }

  // 未儲存偵測：與 config 現值逐欄比對（比對值採 save() 相同的正規化），改幾欄算幾項
  const dirtyCount = [
    apiKey.trim() !== (config.api_keys["openrouter"] ?? ""),
    baseUrl.trim() !== String(config.preferences["base_url"] ?? ""),
    imageModel.trim() !== String(config.preferences["image_model"] ?? ""),
    claudeCompatBaseUrl.trim() !== String(config.preferences["claude_base_url"] ?? ""),
    claudeCompatKey.trim() !== (config.api_keys["claude_compat"] ?? ""),
    gmTier !== String(config.preferences["gm_tier"] ?? "best"),
    String(Math.max(1, Number(maxRound) || 3)) !==
      String(config.preferences["max_round_speakers"] ?? 3),
    transport !== String(config.preferences["transport"] ?? "api"),
    riskAccepted !== (config.preferences["cli_risk_accepted"] === true),
    JSON.stringify(tierModels) !==
      JSON.stringify({ ...SUGGESTED_TIER_MODELS, ...config.tier_models }),
  ].filter(Boolean).length;

  useEffect(() => {
    onDirty(dirtyCount);
    return () => onDirty(0);
  }, [dirtyCount, onDirty]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    if (transport !== "api" && !riskAccepted) {
      setMessage(t("riskRequired"));
      return;
    }
    const next: AppConfig = {
      ...config,
      api_keys: {
        ...config.api_keys,
        openrouter: apiKey.trim(),
        claude_compat: claudeCompatKey.trim(),
      },
      tier_models: tierModels,
      preferences: {
        ...config.preferences,
        base_url: baseUrl.trim(),
        image_model: imageModel.trim(),
        claude_base_url: claudeCompatBaseUrl.trim(),
        transport,
        cli_risk_accepted: riskAccepted,
        gm_tier: gmTier,
        max_round_speakers: Math.max(1, Number(maxRound) || 3),
      },
    };
    try {
      await invoke("write_config", { config: next });
      onSaved(next);
      setMessage(t("saved"));
      if (transport !== "api" && next.preferences[cliNoticeKey(transport)] !== true) {
        setPermissionNotice(transport);
      }
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  // 看過就記下，同一家不再擋；寫失敗就當沒看過（下次再提醒，比默默吞掉好）
  async function ackPermissionNotice() {
    const provider = permissionNotice;
    setPermissionNotice("");
    const next: AppConfig = {
      ...config,
      preferences: { ...config.preferences, [cliNoticeKey(provider)]: true },
    };
    try {
      await invoke("write_config", { config: next });
      onSaved(next);
    } catch {
      /* 記不起來只是下次再問一次，不打斷玩家 */
    }
  }

  return (
    <form id="ai-settings-form" onSubmit={save} className="settings-form">
        <fieldset className="transport-choice">
          <legend>{t("transportLegend")}</legend>
          <label className="inline">
            <input
              type="radio"
              name="transport"
              checked={transport === "api"}
              onChange={() => setTransport("api")}
            />
            {t("transportApi")}
          </label>
          {(["claude", "codex", "agy", "grok"] as const).map((id) => {
            // clis === null＝偵測還沒回來，與「偵測不到」是兩回事：此時不給按鈕，避免誤按一鍵安裝
            const detecting = clis === null;
            const found = clis?.find((c) => c.id === id);
            const progress = installProgress[id];
            const connected = config.preferences[cliConnectedKey(id)] === true;
            return (
              <label key={id} className="inline">
                <input
                  type="radio"
                  name="transport"
                  disabled={!found}
                  checked={transport === id}
                  onChange={() => setTransport(id)}
                />
                {CLI_LABELS[id]}
                {t("cliSubscriptionSuffix")}
                {detecting ? (
                  <span className="cli-version" role="status">
                    {t("cliDetecting")}
                  </span>
                ) : found ? (
                  <>
                    <span className="cli-version">{t("cliDetected", { version: found.version })}</span>
                    {connected && installingCli !== id ? (
                      <>
                        <span className="cli-connected">{t("cliConnectedBadge")}</span>
                        <button
                          type="button"
                          disabled={installingCli !== null && installingCli !== id}
                          onClick={() => void installCli(id)}
                        >
                          {t("cliReverifyBtn")}
                        </button>
                      </>
                    ) : (
                      <button
                        type="button"
                        disabled={installingCli !== null && installingCli !== id}
                        onClick={() => void installCli(id)}
                      >
                        {installingCli === id
                          ? t("cliInstalling", { provider: CLI_LABELS[id] })
                          : t("cliLoginVerifyBtn")}
                      </button>
                    )}
                  </>
                ) : (
                  <>
                    <span className="cli-version">{t("cliNotDetected")}</span>
                    <button
                      type="button"
                      disabled={installingCli !== null && installingCli !== id}
                      onClick={() => void installCli(id)}
                    >
                      {installingCli === id
                        ? t("cliInstalling", { provider: CLI_LABELS[id] })
                        : t("cliInstallBtn")}
                    </button>
                  </>
                )}
                {progress && (
                  <span
                    className={`cli-install-progress${progress.stage === "error" ? " cli-install-error" : ""}`}
                    role={progress.stage === "error" ? "alert" : "status"}
                  >
                    <strong>{cliInstallStageText(progress.stage)}</strong>
                    {progress.stage === "error" && cliInstallErrorHint(progress.detail) && (
                      <span className="cli-install-hint">{cliInstallErrorHint(progress.detail)}</span>
                    )}
                    {progress.detail && (
                      <span className="cli-install-detail">{progress.detail}</span>
                    )}
                    {progress.logPath && (
                      <small>{t("cliInstallLogPath", { path: progress.logPath })}</small>
                    )}
                  </span>
                )}
              </label>
            );
          })}
        </fieldset>
        {transport !== "api" && (
          <div className="risk-box" role="note">
            <strong>{t("riskTitle")}</strong>
            <ul>
              {CLI_RISK_KEYS.map((key) => (
                <li key={key}>{t(key)}</li>
              ))}
            </ul>
            <label className="inline">
              <input
                type="checkbox"
                checked={riskAccepted}
                onChange={(e) => setRiskAccepted(e.currentTarget.checked)}
              />
              {t("riskAccept")}
            </label>
          </div>
        )}
        {transport !== "api" && (
          <p className="cli-permission-note" role="note">
            {t("cliPermissionNote", { provider: CLI_LABELS[transport] ?? transport })}
          </p>
        )}
        {/* 每家 CLI 第一次啟用時擋一次：此時 CLI 還沒被叫起來，玩家先知道等一下的彈窗是誰在問 */}
        {permissionNotice && (
          <div className="modal-overlay" onClick={() => void ackPermissionNotice()}>
            <div
              className="modal"
              role="dialog"
              aria-modal="true"
              aria-label={t("cliPermissionTitle")}
              onClick={(event) => event.stopPropagation()}
            >
              <h2>{t("cliPermissionTitle")}</h2>
              <p>{t("cliPermissionNote", { provider: CLI_LABELS[permissionNotice] ?? permissionNotice })}</p>
              <div className="ai-gen-footer">
                <button type="button" onClick={() => void ackPermissionNotice()}>
                  {t("cliPermissionAck")}
                </button>
              </div>
            </div>
          </div>
        )}
        {/* OpenRouter 專屬欄位只在 API 直連時顯示，避免 CLI 使用者誤以為必填 */}
        {transport === "api" && (
          <label>
            {t("apiKeyLabel")}
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.currentTarget.value)}
              placeholder="sk-or-..."
            />
          </label>
        )}
        {transport === "api" ? (
          <>
            <label>
              {t("imageModelLabel")}
              <input value={imageModel} onChange={(e) => setImageModel(e.currentTarget.value)} />
            </label>
            {(["best", "balanced", "fast"] as const).map((tier) => (
              <label key={tier}>
                {t("tierModelApiLabel", { tier: tierLabel(tier) })}
                <input
                  list="openrouter-models"
                  value={tierModels[tier] ?? ""}
                  onChange={(e) =>
                    setTierModels({ ...tierModels, [tier]: e.currentTarget.value })
                  }
                />
              </label>
            ))}
            <datalist id="openrouter-models">
              {(catalogs["api"] ?? []).map((m) => (
                <option key={m.id} value={m.id}>
                  {m.label}
                </option>
              ))}
            </datalist>
          </>
        ) : (
          <>
            {(["best", "balanced", "fast"] as const).map((tier) => {
              const key = `${transport}:${tier}`;
              const value = tierModels[key] ?? "";
              const catalog = catalogs[transport] ?? [];
              const custom =
                customTiers[key] ?? (value !== "" && !catalog.some((m) => m.id === value));
              return (
                <label key={key}>
                  {t("tierModelCliLabel", { tier: tierLabel(tier) })}
                  <select
                    value={custom ? "__custom__" : value}
                    onChange={(e) => {
                      const next = e.currentTarget.value;
                      if (next === "__custom__") {
                        setCustomTiers({ ...customTiers, [key]: true });
                      } else {
                        setCustomTiers({ ...customTiers, [key]: false });
                        setTierModels({ ...tierModels, [key]: next });
                      }
                    }}
                  >
                    <option value="">{t("cliDefaultOption")}</option>
                    {catalog.map((m) => (
                      <option key={m.id} value={m.id}>
                        {m.label}
                      </option>
                    ))}
                    <option value="__custom__">{t("customModelOption")}</option>
                  </select>
                  {custom && (
                    <input
                      value={value}
                      placeholder={t("customModelPlaceholder")}
                      onChange={(e) =>
                        setTierModels({ ...tierModels, [key]: e.currentTarget.value })
                      }
                    />
                  )}
                </label>
              );
            })}
            <p className="cli-version" role="note">
              {transport === "claude"
                ? t("cliCatalogClaude")
                : transport === "agy"
                  ? t("cliCatalogAgy")
                  : transport === "grok"
                    ? t("cliCatalogGrok")
                  : t("cliCatalogCodex")}
            </p>
          </>
        )}
        {transport === "claude" && (
          <details>
            <summary>{t("claudeCompatSummary")}</summary>
            <label>
              {t("claudeCompatBaseUrlLabel")}
              <input
                value={claudeCompatBaseUrl}
                onChange={(e) => setClaudeCompatBaseUrl(e.currentTarget.value)}
                placeholder="https://api.example.com"
              />
            </label>
            <label>
              {t("claudeCompatKeyLabel")}
              <input
                type="password"
                value={claudeCompatKey}
                onChange={(e) => setClaudeCompatKey(e.currentTarget.value)}
              />
            </label>
            <p role="note">{t("claudeCompatHint")}</p>
          </details>
        )}
        <label>
          {t("gmTierLabel")}
          <select value={gmTier} onChange={(e) => setGmTier(e.currentTarget.value)}>
            {(["best", "balanced", "fast"] as const).map((tier) => (
              <option key={tier} value={tier}>
                {tierLabel(tier)}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t("maxRoundLabel")}
          <input
            type="number"
            min={1}
            max={10}
            value={maxRound}
            onChange={(e) => setMaxRound(e.currentTarget.value)}
          />
        </label>
        {transport === "api" && (
          <label>
            {t("baseUrlLabel")}
            <input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.currentTarget.value)}
              placeholder="https://openrouter.ai/api/v1"
            />
          </label>
        )}
      {message && (
        <div className="row">
          <span role="status">{message}</span>
        </div>
      )}
    </form>
  );
}

// 文字大小偏好：存 config.preferences.text_size，套在 html 根字級（rem 版面跟著縮放）。
// 五檔偏小取向（大螢幕看長文要小字）；預設 l（16px）＝原本的視覺大小
const TEXT_SIZE_PX: Record<string, string> = {
  xs: "10px",
  s: "12px",
  m: "14px",
  l: "16px",
  xl: "18px",
};
const TEXT_SIZE_LABEL_KEYS = {
  xs: "textSizeXS",
  s: "textSizeS",
  m: "textSizeM",
  l: "textSizeL",
  xl: "textSizeXL",
} as const;
const TEXT_SIZE_DEFAULT = "l";

type UsageRow = {
  source: string;
  model: string;
  rounds: number;
  prompt_tokens: number;
  cached_tokens: number;
  output_tokens: number;
  hit_rate: number;
  cost_usd: number | null;
  cost_partial: boolean;
  saved_tokens: number;
  priced_tokens: number;
  saved_usd: number | null;
  saved_partial: boolean;
  unreported: number;
  in_use: boolean;
};
type UsageReport = {
  worlds: { id: string; name: string; rounds: number }[];
  rows: UsageRow[];
  total: UsageRow;
  ping: UsageRow;
  diags: { diag: string; rounds: number }[];
  latest: { ts: string; diag: string; reason: string | null } | null;
};

// 診斷標籤字典（後端 usage_log.rs 模組頂註解那張表）：短名給統計、長句給最近一輪。
// 燈號純規則：正常綠、暖機／過期黃、該中沒中紅、單發模式不打分。
const DIAG_KEYS = {
  ok: ["usageDiagOk", "usageWhyOk", "good"],
  ping: ["usageDiagPing", "usageWhyPing", "good"],
  warmup: ["usageDiagWarmup", "usageWhyWarmup", "warn"],
  expired: ["usageDiagExpired", "usageWhyExpired", "warn"],
  single: ["usageDiagSingle", "usageWhySingle", "idle"],
  "prefix-broken": ["usageDiagPrefixBroken", "usageWhyPrefixBroken", "bad"],
  "cache-skipped": ["usageDiagCacheSkipped", "usageWhyCacheSkipped", "bad"],
  "no-cache": ["usageDiagNoCache", "usageWhyNoCache", "bad"],
  "drop-lane": ["usageDiagDropLane", "usageWhyDropLane", "bad"],
} as const;
const REASON_KEYS = {
  "first-turn": "usageReasonFirstTurn",
  "pending-rewrite": "usageReasonPendingRewrite",
  "scene-changed": "usageReasonSceneChanged",
  "history-rewound": "usageReasonHistoryRewound",
  "history-edited": "usageReasonHistoryEdited",
  "reply-diverged": "usageReasonReplyDiverged",
  "resume-failed": "usageReasonResumeFailed",
  "rewrite-failed": "usageReasonRewriteFailed",
  "ping-truncate-failed": "usageReasonPingTruncateFailed",
} as const;

function diagEntry(diag: string) {
  return DIAG_KEYS[diag as keyof typeof DIAG_KEYS];
}

function tokens(value: number) {
  return value.toLocaleString();
}

// 細項的花費是 CLI 官方回報值，收合處的已省金額是後端拿它估的；兩邊都可能只湊到一部分（標「≥」）。
// 已省在建快取那幾輪還沒回本會是負的，對外收到 0 就好，別讓玩家看到「省下 -$0.003」
function money(value: number | null, partial: boolean) {
  if (value === null) return "—";
  return `${partial ? "≥ " : ""}$${Math.max(0, value).toFixed(3)}`;
}

// 額度分頁（快取包 6）：讀後端彙總好的 prompt-cache.jsonl，以桌為主視圖、桌內按模型分行
function UsageTab({ currentWorld }: { currentWorld: string }) {
  // null＝所有桌總計；"" ＝未標桌（加桌欄位之前的舊紀錄）
  const [scope, setScope] = useState<string | null>(currentWorld || null);
  const [report, setReport] = useState<UsageReport | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    let stale = false;
    invoke<UsageReport>("usage_report", { worldId: scope })
      .then((loaded) => {
        if (!stale) setReport(loaded);
      })
      .catch((reason) => {
        if (!stale) setError(String(reason));
      });
    return () => {
      stale = true;
    };
  }, [scope]);

  if (error) return <ErrorNote text={error} />;
  if (!report) return null;
  if (report.worlds.length === 0) return <p className="usage-empty">{t("usageEmpty")}</p>;

  // 「最近一輪怎麼樣」是某一桌的事；看總計時沒有「最近一輪」這回事，不出這句話
  const latest = scope === null ? null : report.latest;
  const entry = latest ? diagEntry(latest.diag) : undefined;
  const reasonKey = latest?.reason ? REASON_KEYS[latest.reason as keyof typeof REASON_KEYS] : undefined;
  // 非綠燈＝值得在收合狀態就看到；正常與單發留在細項裡就好
  const alert = entry && latest && entry[2] !== "good" && entry[2] !== "idle";
  const bodyRows = [...report.rows];
  if (report.ping.rounds > 0) bodyRows.push({ ...report.ping, source: "", model: "ping" });
  // 第一眼只講省下多少（總用量與花費留在細項）。保溫也是真的花掉的錢，一併算進來
  const totals = {
    prompt_tokens: report.total.prompt_tokens + report.ping.prompt_tokens,
    cached_tokens: report.total.cached_tokens + report.ping.cached_tokens,
    saved_tokens: report.total.saved_tokens + report.ping.saved_tokens,
    priced_tokens: report.total.priced_tokens + report.ping.priced_tokens,
    saved_usd:
      report.total.saved_usd === null && report.ping.saved_usd === null
        ? null
        : (report.total.saved_usd ?? 0) + (report.ping.saved_usd ?? 0),
    saved_partial: report.total.saved_partial || report.ping.saved_partial,
  };
  const hit = totals.prompt_tokens === 0 ? 0 : (totals.cached_tokens * 100) / totals.prompt_tokens;
  const savedPct =
    totals.priced_tokens === 0 ? 0 : Math.max(0, (totals.saved_tokens * 100) / totals.priced_tokens);
  // 金額湊不齊（混了不回報用量或計價不明的來源）就不出現在第一眼，改由細項逐列交代
  const showSavedUsd = totals.saved_usd !== null && !totals.saved_partial;

  return (
    <div className="usage-tab">
      <label className="usage-scope">
        {t("usageScopeLabel")}
        <select
          value={scope ?? "*"}
          onChange={(event) => setScope(event.currentTarget.value === "*" ? null : event.currentTarget.value)}
        >
          <option value="*">{t("usageAllTables")}</option>
          {report.worlds.map((world) => (
            <option key={world.id} value={world.id}>
              {world.name || (world.id ? t("usageDeletedTable") : t("usageGenesis"))}
            </option>
          ))}
        </select>
      </label>

      {bodyRows.length === 0 && <p className="usage-empty">{t("usageEmpty")}</p>}

      {totals.prompt_tokens > 0 && (
        <>
          {totals.priced_tokens > 0 && (
            <p className="usage-headline">
              <span className="usage-headline-saved">
                {t("usageSavedHeadline", { pct: savedPct.toFixed(0) })}
              </span>
              {showSavedUsd && (
                <span className="usage-headline-cost">
                  {t("usageSavedAbout")} {money(totals.saved_usd, false)}
                </span>
              )}
            </p>
          )}
          <div
            className="usage-bar"
            role="img"
            aria-label={`${t("usageBarHit")} ${hit.toFixed(0)}%`}
          >
            <span className="usage-bar-hit" style={{ width: `${hit}%` }} title={t("usageBarHit")}>
              {hit >= 18 && `${t("usageBarHit")} ${hit.toFixed(0)}%`}
            </span>
            <span className="usage-bar-full" style={{ width: `${100 - hit}%` }} title={t("usageBarFull")}>
              {hit <= 82 && `${t("usageBarFull")} ${(100 - hit).toFixed(0)}%`}
            </span>
          </div>
        </>
      )}

      {/* 狀況不正常時在收合狀態出聲，正常就只留在細項裡——同一句話不重複出現兩次 */}
      {alert && (
        <p className={`usage-latest usage-${entry[2]}`}>
          <span className="usage-dot" aria-hidden="true" />
          <strong>{t(entry[0])}</strong>
          {" — "}
          {t(entry[1])}
          {reasonKey && `（${t(reasonKey)}）`}
        </p>
      )}

      {bodyRows.length > 0 && (
      <details className="usage-details">
        <summary>{t("usageDetailsToggle")}</summary>

        {entry && latest && !alert && (
          <p className={`usage-latest usage-${entry[2]}`}>
            <span className="usage-dot" aria-hidden="true" />
            <strong>{t(entry[0])}</strong>
            {" — "}
            {t(entry[1])}
            {reasonKey && `（${t(reasonKey)}）`}
            <span className="usage-latest-ts">{latest.ts}</span>
          </p>
        )}

        <div className="usage-table-wrap">
          <table className="usage-table">
            <thead>
              <tr>
                <th>{t("usageModel")}</th>
                <th>{t("usageRounds")}</th>
                <th>{t("usageInputTokens")}</th>
                <th>{t("usageCached")}</th>
                <th>{t("usageHitRate")}</th>
                <th>{t("usageOutput")}</th>
                <th>{t("usageCost")}</th>
              </tr>
            </thead>
            <tbody>
              {bodyRows.map((row) => (
                <tr key={`${row.source}/${row.model}`} className={row.model === "ping" ? "usage-ping-row" : undefined}>
                  <th scope="row">
                    {row.model === "ping" ? (
                      t("usagePing")
                    ) : (
                      <>
                        <span className="usage-source">{row.source}</span> {row.model}
                        {row.in_use && <span className="usage-badge">{t("usageInUse")}</span>}
                      </>
                    )}
                  </th>
                  <td>{row.rounds}</td>
                  <td colSpan={row.unreported === row.rounds ? 4 : 1}>
                    {row.unreported === row.rounds ? (
                      <span className="usage-muted">{t("usageNoUsage")}</span>
                    ) : (
                      tokens(row.prompt_tokens)
                    )}
                  </td>
                  {row.unreported !== row.rounds && (
                    <>
                      <td>{tokens(row.cached_tokens)}</td>
                      <td>{row.hit_rate.toFixed(1)}%</td>
                      <td>{tokens(row.output_tokens)}</td>
                    </>
                  )}
                  <td>{money(row.cost_usd, row.cost_partial)}</td>
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr>
                <th scope="row">{t("usageTotal")}</th>
                <td>{report.total.rounds}</td>
                <td>{tokens(report.total.prompt_tokens)}</td>
                <td>{tokens(report.total.cached_tokens)}</td>
                <td>{report.total.hit_rate.toFixed(1)}%</td>
                <td>{tokens(report.total.output_tokens)}</td>
                <td>{money(report.total.cost_usd, report.total.cost_partial)}</td>
              </tr>
            </tfoot>
          </table>
        </div>

        <p className="usage-diags">
          {report.diags.map((count) => {
            const label = diagEntry(count.diag);
            return label ? (
              <span key={count.diag} className={`usage-chip usage-${label[2]}`}>
                {t(label[0])} ×{count.rounds}
              </span>
            ) : null;
          })}
        </p>
        <p className="usage-note">{t("usageCostNote")}</p>
      </details>
      )}
    </div>
  );
}

// 單一設定入口內分頁（NewPlan §9.4）：外觀為預設頁，不碰 AI 的人打開只見外觀
function SettingsWindow({
  config,
  onSaved,
  onPreference,
  sponsorUnlocked,
  onSponsorUnlocked,
  onClose,
  initialTab = "appearance",
  currentWorld,
}: {
  config: AppConfig;
  onSaved: (c: AppConfig) => void;
  onPreference: (key: string, value: unknown) => void;
  sponsorUnlocked: boolean;
  onSponsorUnlocked: () => void;
  onClose: () => void;
  initialTab?: "appearance" | "ai" | "author";
  currentWorld: string;
}) {
  const [tab, setTab] = useState<"appearance" | "ai" | "usage" | "author">(initialTab);
  const [previewTheme, setPreviewTheme] = useState<ThemeId | null>(null);
  const [sponsorPackError, setSponsorPackError] = useState("");
  const sponsorPackInputRef = useRef<HTMLInputElement>(null);
  // AI 分頁的未儲存欄位數（外觀分頁即改即存，恆為 0）
  const [dirtyCount, setDirtyCount] = useState(0);

  // 有未儲存修改時先確認再離開；返回 true 表示可以離開
  async function confirmDiscard() {
    if (dirtyCount === 0) return true;
    return confirm(t("unsavedLeaveConfirm", { n: dirtyCount }), {
      title: t("unsavedLeaveTitle"),
      kind: "warning",
    });
  }

  async function discardAndClose() {
    if (await confirmDiscard()) onClose();
  }

  async function switchTab(target: "appearance" | "usage" | "author") {
    if (await confirmDiscard()) setTab(target);
  }

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") void discardAndClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const textSize = String(config.preferences["text_size"] ?? TEXT_SIZE_DEFAULT);
  const selectedTheme = previewTheme ?? resolveTheme(config, sponsorUnlocked);

  useEffect(() => {
    document.documentElement.dataset.theme = previewTheme ?? resolveTheme(config, sponsorUnlocked);
    return () => {
      document.documentElement.dataset.theme = resolveTheme(config, sponsorUnlocked);
    };
  }, [previewTheme, config, sponsorUnlocked]);

  async function importSponsorPack(file: File) {
    setSponsorPackError("");
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      await invoke("import_sponsor_pack", { data: Array.from(bytes) });
      onSponsorUnlocked();
    } catch (reason) {
      setSponsorPackError(String(reason));
    }
  }

  function selectTheme(theme: ThemeId) {
    if ((SPONSOR_THEMES as readonly string[]).includes(theme) && !sponsorUnlocked) {
      setPreviewTheme(theme);
      return;
    }
    setPreviewTheme(null);
    onPreference("theme", theme);
  }

  return (
    <div className="modal-overlay" onClick={() => void discardAndClose()}>
      <div
        className="modal"
        role="dialog"
        aria-label={t("settingsBtn")}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="modal-header">
          <nav className="tabs" aria-label={t("settingsBtn")}>
            <button
              className={tab === "appearance" ? "tab tab-active" : "tab"}
              onClick={() => void switchTab("appearance")}
            >
              {t("appearanceTab")}
            </button>
            <button className={tab === "ai" ? "tab tab-active" : "tab"} onClick={() => setTab("ai")}>
              {t("aiTab")}
            </button>
            <button
              className={tab === "usage" ? "tab tab-active" : "tab"}
              onClick={() => void switchTab("usage")}
            >
              {t("usageTab")}
            </button>
            <button
              className={tab === "author" ? "tab tab-active" : "tab"}
              onClick={() => void switchTab("author")}
            >
              {t("authorTab")}
            </button>
          </nav>
          <div className="row">
            {dirtyCount > 0 && (
              <span className="unsaved-hint" role="status">
                {t("unsavedChanges", { n: dirtyCount })}
              </span>
            )}
            {tab === "ai" && (
              <button type="submit" form="ai-settings-form">
                {t("saveSettings")}
              </button>
            )}
            <button onClick={() => void discardAndClose()}>
              {dirtyCount > 0 ? t("settingsDiscard") : t("settingsBack")}
            </button>
          </div>
        </header>
        {tab === "appearance" ? (
          <div className="settings-form">
            <label>
              {t("languageLabel")}
              <select
                value={normalizeLang(config.preferences["language"])}
                onChange={(e) => onPreference("language", normalizeLang(e.currentTarget.value))}
              >
                {LANGUAGE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <div className="theme-setting">
              {t("themeLabel")}
              <div className="theme-swatches">
                {ALL_THEMES.map((theme) => {
                  const locked = (SPONSOR_THEMES as readonly string[]).includes(theme) && !sponsorUnlocked;
                  const name = t(THEME_LABEL_KEYS[theme]);
                  return (
                    <button
                      key={theme}
                      type="button"
                      className="theme-swatch"
                      aria-pressed={selectedTheme === theme}
                      title={name}
                      onClick={() => selectTheme(theme)}
                    >
                      <span
                        className={selectedTheme === theme ? "swatch-chip swatch-chip-selected" : "swatch-chip"}
                        style={{ backgroundColor: THEME_SWATCH[theme].bg }}
                      >
                        {locked && <span className="swatch-kofi">☕</span>}
                        <span className="swatch-dot" style={{ backgroundColor: THEME_SWATCH[theme].dot }} />
                      </span>
                      <span>{name}</span>
                    </button>
                  );
                })}
              </div>
              {previewTheme && (
                <p className="theme-preview-hint">
                  {t("themePreviewHint", { name: t(THEME_LABEL_KEYS[previewTheme]) })}{" "}
                  <button type="button" className="link" onClick={() => void openUrl(KOFI_URL)}>
                    {t("sponsorBtn")}
                  </button>
                </p>
              )}
            </div>
            <label>
              {t("textSizeLabel")}
              <select
                value={textSize in TEXT_SIZE_PX ? textSize : TEXT_SIZE_DEFAULT}
                onChange={(e) => onPreference("text_size", e.currentTarget.value)}
              >
                {(["xs", "s", "m", "l", "xl"] as const).map((size) => (
                  <option key={size} value={size}>
                    {t(TEXT_SIZE_LABEL_KEYS[size])}
                  </option>
                ))}
              </select>
            </label>
          </div>
        ) : tab === "usage" ? (
          <UsageTab currentWorld={currentWorld} />
        ) : tab === "author" ? (
          <div className="author-page">
            <img src={taoIcon} alt="TaoGongSun" className="avatar-round author-avatar" />
            <strong>TaoGongSun</strong>
            <p className="author-blurb">{t("authorBlurb")}</p>
            <button type="button" onClick={() => void openUrl(KOFI_URL)}>
              {t("sponsorBtn")}
            </button>
            {sponsorUnlocked ? (
              <p role="status">{t("sponsorPackUnlocked")}</p>
            ) : (
              <>
                <button type="button" onClick={() => sponsorPackInputRef.current?.click()}>
                  {t("importSponsorPack")}
                </button>
                <input
                  ref={sponsorPackInputRef}
                  type="file"
                  accept=".ttpack"
                  hidden
                  onChange={(event) => {
                    const file = event.currentTarget.files?.[0];
                    event.currentTarget.value = "";
                    if (file) void importSponsorPack(file);
                  }}
                />
                {sponsorPackError && <small role="alert">{t("sponsorPackImportError", { reason: sponsorPackError })}</small>}
              </>
            )}
          </div>
        ) : (
          <Settings config={config} onSaved={onSaved} onDirty={setDirtyCount} />
        )}
      </div>
    </div>
  );
}

// 拖曳排序：按住移動超過門檻才算拖曳，門檻內放開仍是單純點擊（角色卡的點擊＝選發言者）
const DRAG_THRESHOLD_PX = 5;

function useDragReorder<T>(
  items: T[],
  keyOf: (item: T) => string,
  onReorder: (ordered: T[]) => void,
) {
  const [preview, setPreview] = useState<T[] | null>(null);
  const [draggingKey, setDraggingKey] = useState<string | null>(null);
  const rows = useRef(new Map<string, HTMLElement>());
  const dragged = useRef(false);

  // 一次只跟相鄰那列交換：越過鄰居中線就換，換完中線落到指標另一側，高度不一也不會來回抖
  function neighbourStep(y: number, order: T[], from: number): number {
    const midpoint = (index: number) => {
      const item = order[index];
      const row = item === undefined ? undefined : rows.current.get(keyOf(item));
      if (!row) return null;
      const rect = row.getBoundingClientRect();
      return rect.top + rect.height / 2;
    };
    const above = midpoint(from - 1);
    if (above !== null && y < above) return from - 1;
    const below = midpoint(from + 1);
    if (below !== null && y > below) return from + 1;
    return from;
  }

  function startDrag(event: ReactPointerEvent, item: T) {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest("button, a, input, textarea, select")) return;
    const key = keyOf(item);
    const startY = event.clientY;
    let order = items;
    let started = false;

    const move = (moveEvent: globalThis.PointerEvent) => {
      if (!started) {
        if (Math.abs(moveEvent.clientY - startY) < DRAG_THRESHOLD_PX) return;
        started = true;
        dragged.current = true;
        setDraggingKey(key);
      }
      const from = order.findIndex((candidate) => keyOf(candidate) === key);
      const target = neighbourStep(moveEvent.clientY, order, from);
      if (target === from) return;
      const next = order.slice();
      const [moved] = next.splice(from, 1);
      next.splice(target, 0, moved);
      order = next;
      setPreview(next);
    };
    const finish = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      if (started) {
        onReorder(order);
        // 放開後瀏覽器才補送 click，等它送完再解旗標
        setTimeout(() => (dragged.current = false), 0);
      }
      setDraggingKey(null);
      setPreview(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
  }

  return {
    order: preview ?? items,
    draggingKey,
    justDragged: () => dragged.current,
    rowProps: (item: T) => ({
      onPointerDown: (event: ReactPointerEvent) => startDrag(event, item),
      ref: (element: HTMLElement | null) => {
        if (element) rows.current.set(keyOf(item), element);
        else rows.current.delete(keyOf(item));
      },
    }),
  };
}

// 重構卡存檔對話框預設檔名：桌名可能含檔名非法字元，一律代換成 -；空桌名就不接前綴，只用在地化字尾
function refactorCardFileName(tableName: string): string {
  const safe = tableName.replace(/[\\/:*?"<>|\x00-\x1f\x7f]/g, "-");
  return `${safe ? `${safe}-` : ""}${t("refactorExportFileName")}.json`;
}

// 淘汰理由 rule／稽核 kind 都是後端固定枚舉，本地寫死對照 i18n 鍵；查不到就退第一種，不讓畫面空白。
const REFACTOR_DROPPED_RULE_KEYS: Record<number, MsgKey> = {
  1: "refactorDroppedRule1",
  2: "refactorDroppedRule2",
  3: "refactorDroppedRule3",
  4: "refactorDroppedRule4",
};
const REFACTOR_AUDIT_KIND_KEYS: Record<string, MsgKey> = {
  coverage: "refactorAuditKindCoverage",
  mechanism: "refactorAuditKindMechanism",
  split: "refactorAuditKindSplit",
  drop_rule: "refactorAuditKindDropRule",
  excused: "refactorAuditKindExcused",
};

// 世界書 v1：一份只進 GM 上下文的 world.md（NewPlan §7.0）
function WorldEditor({
  world,
  worldName,
  onBack,
  leaveGuard,
  convertColor,
  onEntryConverted,
  onRefactorApplied,
}: {
  world: string;
  worldName: string;
  onBack: () => void;
  /** 側欄要離開世界設定時先問過這裡（未儲存確認與返回鈕同一條） */
  leaveGuard: { current: (() => Promise<boolean>) | null };
  convertColor: string;
  onEntryConverted: () => Promise<void>;
  /** AI 卡重構套用成功後：角色清單／卡片介面／桌面狀態都可能變了，交回 App 層重載 */
  onRefactorApplied: () => Promise<void>;
}) {
  const [text, setText] = useState<string | null>(null);
  const [savedText, setSavedText] = useState("");
  const [message, setMessage] = useState("");
  const [entries, setEntries] = useState<WorldbookEntry[]>([]);
  const [ledger, setLedger] = useState<Ledger>(EMPTY_LEDGER);
  const [characters, setCharacters] = useState<CharacterMeta[]>([]);
  const [worldbookMessage, setWorldbookMessage] = useState("");
  const [draft, setDraft] = useState<WorldbookDraft | null>(null);
  // 條目表單開啟當下的快照，用來判斷「有沒有改過」（未儲存提示）
  const [draftOrigin, setDraftOrigin] = useState("");
  const draftFormRef = useRef<HTMLFormElement>(null);
  const entryDrag = useDragReorder(
    entries,
    (entry) => String(entry.uid),
    (ordered) => void reorderEntries(ordered),
  );
  // AI 卡重構：結果卡（產物讀進來後的人審／套用）與下面的「盤點→展開」進度是兩段獨立狀態，
  // 交會點是 setRefactorOutcome——AI 兩階段跑完、或選檔路徑讀完 JSON，都寫進同一份結果卡。
  const [refactorOutcome, setRefactorOutcome] = useState<RefactorOutcome | null>(null);
  const [refactorSelection, setRefactorSelection] = useState<RefactorSelection | null>(null);
  const [refactorOrigin, setRefactorOrigin] = useState<"ai" | "import" | null>(null);
  const [refactorDetail, setRefactorDetail] = useState(false);
  // pool 呼叫失敗的條目名單（2026-08-12 B 拍板）：顯示在結果視窗頂部紅字段——以前塞頁面
  // 角落的一行狀態文字，被結果 modal 蓋住玩家看不到。
  const [refactorFailures, setRefactorFailures] = useState<{ name: string; reason: string }[]>([]);
  const [refactorBusy, setRefactorBusy] = useState(false);
  const refactorInputRef = useRef<HTMLInputElement>(null);
  // 非 null＝AI 盤點／展開跑中，modal 顯示 text；cancelling 只管取消鈕的 disabled，不影響迴圈判斷。
  const [refactorProgress, setRefactorProgress] = useState<{ text: string; cancelling: boolean; tail: string } | null>(null);
  // 迴圈裡讀取的取消旗標——用 ref 而非 state：async 迴圈裡的閉包看不到後續 setState，只有 ref.current 每次都讀最新值。
  const refactorCancelRef = useRef(false);

  async function refreshCast() {
    try {
      const cast = await invoke<CharacterMeta[]>("list_characters", { worldId: world });
      setCharacters(cast.filter((character) => !character.archived));
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  useEffect(() => {
    setMessage("");
    setWorldbookMessage("");
    setText(null);
    setEntries([]);
    setLedger(EMPTY_LEDGER);
    setCharacters([]);
    setDraft(null);
    invoke<string>("read_world_md", { worldId: world })
      .then((value) => {
        setText(value);
        setSavedText(value);
      })
      .catch((reason) => setMessage(String(reason)));
    invoke<WorldbookEntry[]>("read_worldbook", { worldId: world })
      .then(setEntries)
      .catch((reason) => setWorldbookMessage(String(reason)));
    // 帳本掛掉不該擋住世界書編輯：失敗就當空，不彈錯誤。
    invoke<Ledger>("mechanism_ledger", { worldId: world })
      .then(setLedger)
      .catch(() => setLedger(EMPTY_LEDGER));
    void refreshCast();
  }, [world]);

  // 新增的空白表單排在清單底部，展開時可能在畫面外，捲到看得見
  // （不用 smooth：長清單的平滑捲動會被後續 render 打斷，停在半路）
  useEffect(() => {
    draftFormRef.current?.scrollIntoView({ block: "nearest" });
  }, [draftOrigin]);

  if (text === null) return message ? <p role="alert">{message}</p> : null;

  const draftDirty = draft !== null && JSON.stringify(draft) !== draftOrigin;
  // 既有條目改到一半離開時會自動存，不算未儲存；只有還沒存過的新條目要提醒
  const newEntryDirty = draftDirty && draft?.uid === null;
  const unsavedCount = (text !== savedText ? 1 : 0) + (newEntryDirty ? 1 : 0);

  async function saveWorldSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    try {
      await invoke("write_world_md", { worldId: world, content: text });
      setSavedText(text ?? "");
      setMessage(t("saved"));
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function confirmLeave() {
    // 開著的既有條目照換編輯對象那套：先存起來再走，存不起來就別走
    if (draft && draftDirty && draft.uid !== null) {
      if (!(await persistDraft(draft))) return false;
      setDraft(null);
    }
    if (unsavedCount === 0) return true;
    return await confirm(t("unsavedLeaveConfirm", { n: unsavedCount }), {
      title: t("unsavedLeaveTitle"),
      kind: "warning",
    });
  }
  // 側欄切走時走的是同一條確認；每次 render 掛上，閉包才拿得到最新的 unsavedCount
  leaveGuard.current = confirmLeave;

  async function handleBack() {
    if (await confirmLeave()) onBack();
  }

  async function refreshWorldbook() {
    setEntries(await invoke<WorldbookEntry[]>("read_worldbook", { worldId: world }));
  }

  async function refreshLedger() {
    try {
      setLedger(await invoke<Ledger>("mechanism_ledger", { worldId: world }));
    } catch {
      setLedger(EMPTY_LEDGER);
    }
  }

  // 帳本的「照原文送模型」開關＝重用既有 upsert_worldbook_entry 反轉該條目的 disabled；
  // 找不到該 uid 就跳過（條目已被刪，不是這裡的錯）。
  async function toggleLedgerEntry(ledgerEntry: LedgerEntry) {
    const target = entries.find((entry) => entry.uid === ledgerEntry.uid);
    if (!target || target.locked) return;
    setWorldbookMessage("");
    try {
      await invoke<number>("upsert_worldbook_entry", {
        worldId: world,
        entry: { ...target, disabled: !target.disabled },
      });
      await refreshWorldbook();
      await refreshLedger();
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  // 條目表單按取消＝丟資料，先問過（自動存只走切換編輯對象那條路）
  async function confirmDiscardDraft() {
    if (!draftDirty) return true;
    return await confirm(t("unsavedLeaveConfirm", { n: 1 }), {
      title: t("unsavedLeaveTitle"),
      kind: "warning",
    });
  }

  // 換編輯對象＝把手上這條存起來就走（條目本來就是即時寫檔，多問一次只是擋路）。
  // 還沒存過的新條目例外：直接存會把半成品留在清單上，照舊問。
  async function openDraft(next: WorldbookDraft) {
    let autoSaved = false;
    if (draft && draftDirty) {
      if (draft.uid === null) {
        if (!(await confirmDiscardDraft())) return;
      } else {
        if (!(await persistDraft(draft))) return;
        autoSaved = true;
      }
    }
    setWorldbookMessage(autoSaved ? t("worldbookEntrySaved") : "");
    setDraft(next);
    setDraftOrigin(JSON.stringify(next));
  }

  async function closeDraft() {
    if (await confirmDiscardDraft()) setDraft(null);
  }

  function addEntry() {
    void openDraft({
      uid: null,
      title: "",
      keys: "",
      content: "",
      constant: false,
      enabled: true,
      order: 100,
      visibility: "gm",
      characters: [],
    });
  }

  function editEntry(entry: WorldbookEntry) {
    void openDraft({
      uid: entry.uid,
      title: entry.title,
      keys: entry.keys.join("、"),
      content: entry.content,
      constant: entry.constant,
      enabled: !entry.disabled,
      order: entry.order,
      visibility: entry.visibility.type,
      characters: entry.visibility.type === "characters" ? entry.visibility.characters : [],
    });
  }

  /** 把表單寫回世界書；失敗時把原因留在清單訊息列並回傳 false（表單不關） */
  async function persistDraft(source: WorldbookDraft) {
    const visibility: Visibility =
      source.visibility === "characters"
        ? {
            type: "characters",
            characters: source.characters.filter((id) =>
              characters.some((character) => character.id === id),
            ),
          }
        : { type: source.visibility };
    const entry: WorldbookEntry = {
      uid: source.uid ?? Number.MAX_SAFE_INTEGER,
      title: source.title.trim(),
      keys: source.keys
        .split(/[,、]/)
        .map((key) => key.trim())
        .filter(Boolean),
      content: source.content,
      constant: source.constant,
      order: source.order,
      disabled: !source.enabled,
      locked: false,
      visibility,
    };
    try {
      await invoke<number>("upsert_worldbook_entry", { worldId: world, entry });
      await refreshWorldbook();
      return true;
    } catch (reason) {
      setWorldbookMessage(String(reason));
      return false;
    }
  }

  async function saveEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft) return;
    setWorldbookMessage("");
    if (!(await persistDraft(draft))) return;
    setDraft(null);
    setWorldbookMessage(t("worldbookEntrySaved"));
  }

  async function deleteEntry(entry: WorldbookEntry) {
    setWorldbookMessage("");
    try {
      const accepted = await confirm(
        t("worldbookDeleteConfirm", { title: entry.title || String(entry.uid) }),
        { title: t("worldbookDeleteTitle"), kind: "warning" },
      );
      if (!accepted) return;
      await invoke("delete_worldbook_entry", { worldId: world, uid: entry.uid });
      await refreshWorldbook();
      if (draft?.uid === entry.uid) setDraft(null);
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  async function reorderEntries(ordered: WorldbookEntry[]) {
    setWorldbookMessage("");
    const previous = entries;
    setEntries(ordered);
    try {
      await invoke("reorder_worldbook_entries", {
        worldId: world,
        uids: ordered.map((entry) => entry.uid),
      });
    } catch (reason) {
      setEntries(previous);
      setWorldbookMessage(String(reason));
    }
  }

  // 去重上線前重複匯入過的桌，用這顆自己收拾：同內容只留排最前面那條
  async function dedupeWorldbook() {
    setWorldbookMessage("");
    try {
      const accepted = await confirm(t("worldbookDedupeConfirm"), {
        title: t("worldbookDedupe"),
        kind: "warning",
      });
      if (!accepted) return;
      // 去重只刪東西，別觸發匯入後的選 GM／改桌名
      const removed = await invoke<number>("dedupe_worldbook", { worldId: world });
      if (removed > 0) await refreshWorldbook();
      setWorldbookMessage(
        removed > 0 ? t("worldbookDedupeDone", { n: removed }) : t("worldbookDedupeNone"),
      );
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  async function exportWorldbook() {
    setWorldbookMessage("");
    try {
      const path = await saveDialog({
        defaultPath: "worldbook.json",
        filters: [{ name: t("worldbookJson"), extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("export_worldbook", { worldId: world, path });
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  // 匯出這桌先前套用過的重構產物（apply() 落檔），重玩同一張卡不必再燒 AI 額度重新展開。
  async function exportSavedRefactorOutcome() {
    setWorldbookMessage("");
    try {
      const path = await saveDialog({
        defaultPath: refactorCardFileName(worldName),
        filters: [{ name: t("refactorOutcomeJson"), extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("refactor_export_saved", { worldId: world, path });
      await revealItemInDir(path);
    } catch (reason) {
      setWorldbookMessage(
        String(reason).includes("refactor-export-none") ? t("refactorExportNone") : String(reason),
      );
    }
  }

  // AI 卡重構：盤點出六區塊小抄（PERSONS／INTERFACE／ENTRIES／SPLITS／GROUPS／FIELDS）→本地
  // 零呼叫組裝（refactor_assemble_local：carry 照搬＋split 零呼叫路由＋clean 人物組卡）→剩餘
  // AI 呼叫全並行（人物佇列＋absorb＋group＋statusbar＋interface，上限 4、無序列鏈）→組產物。
  // knownFields 是 survey.fields 固定一份，所有呼叫共用，不再沿呼叫鏈累積。
  async function runAiRefactor() {
    if (refactorProgress) return;
    if (await invoke<boolean>("refactor_outcome_exists", { worldId: world })) {
      const rerun = await confirm(t("refactorRerunWarnBody"), {
        title: t("refactorBtn"),
        kind: "warning",
      });
      if (!rerun) return;
    }
    setWorldbookMessage("");
    refactorCancelRef.current = false;
    setRefactorProgress({ text: t("refactorSurveying"), cancelling: false, tail: "" });
    try {
      // 共用 tail：所有呼叫（survey＋展開）的 Channel onDelta 都 append 進同一個 buffer，
      // 任一路增量＝活著訊號，不因並行而互相蓋掉彼此的畫面。
      let tailBuffer = "";
      const appendTail = (delta: string) => {
        tailBuffer = (tailBuffer + delta).slice(-2000);
        setRefactorProgress((current) =>
          current && { ...current, tail: tailBuffer.split("\n").slice(-4).join("\n") },
        );
      };
      const makeOnDelta = () => {
        const channel = new Channel<string>();
        channel.onmessage = appendTail;
        return channel;
      };

      const survey = await invoke<RefactorSurveyOutcome>("refactor_survey", { worldId: world, onDelta: makeOnDelta() });
      // 本地零呼叫組裝：carry／split 各路由／clean 人物，毫秒級、不算進並行呼叫額度。
      const local = await invoke<RefactorLocalAssembly>("refactor_assemble_local", { worldId: world, survey });
      const { local: localPersons, queue } = buildRefactorPersonPlan(survey, entries, local.clean_person_names);

      const absorbUids = survey.verdicts.filter((verdict) => verdict.action === "absorb").map((verdict) => verdict.uid);
      // statusbar 段依來源 uid 分組：同一條原始條目的多個 statusbar span 合成一次呼叫。
      const statusbarByUid = new Map<string, string[]>();
      for (const route of survey.splits) {
        if (route.route !== "statusbar") continue;
        const uid = route.span.split("#")[0];
        statusbarByUid.set(uid, [...(statusbarByUid.get(uid) ?? []), route.span]);
      }

      // 全部呼叫進同一個 pool，上限 4 有界並行；不再有「重寫→介面」序列鏈。
      type RefactorTask =
        | { kind: "person"; item: RefactorPersonQueueItem }
        | { kind: "absorb"; uid: string }
        | { kind: "group"; group: RefactorSplitGroup }
        | { kind: "statusbar"; uid: string; spans: string[] }
        | { kind: "interface"; uid: string };
      const pool: RefactorTask[] = [
        ...queue.map((item): RefactorTask => ({ kind: "person", item })),
        ...absorbUids.map((uid): RefactorTask => ({ kind: "absorb", uid })),
        ...survey.groups.map((group): RefactorTask => ({ kind: "group", group })),
        ...[...statusbarByUid.entries()].map(([uid, spans]): RefactorTask => ({ kind: "statusbar", uid, spans })),
        ...survey.interface_uids.map((uid): RefactorTask => ({ kind: "interface", uid })),
      ];
      const totalSteps = pool.length;

      const characters: RefactorCharacter[] = [...local.characters, ...localPersons];
      const refactorEntries: RefactorNewEntry[] = [...local.entries];
      if (totalSteps === 0 && characters.length === 0 && refactorEntries.length === 0) {
        setRefactorProgress(null);
        setWorldbookMessage(t("refactorNothingToDo"));
        return;
      }

      const interfaces: RefactorInterface[] = [];
      // reason 帶原始錯誤文字（去重顯示在結果視窗）：玩家看得到「模型呼叫失敗」這類可修正原因。
      const failedTitles: { name: string; reason: string }[] = [];
      const knownFields = survey.fields; // 命名唯一權威，全呼叫共用同一份、不累積。
      let done = 0;
      const bumpDone = () => {
        done++;
        setRefactorProgress((current) => current && { ...current, text: t("refactorParallelStep", { done, total: totalSteps }) });
      };

      setRefactorProgress((current) => current && { ...current, text: t("refactorParallelStep", { done, total: totalSteps }) });

      const run = async (task: RefactorTask): Promise<void> => {
        const name = task.kind === "person" ? task.item.name : task.kind === "group" ? task.group.title : sourceEntryTitle(entries, task.uid);
        try {
          if (task.kind === "person") {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorPersonExpandOutcome>("refactor_expand_person", {
                  worldId: world,
                  name: task.item.name,
                  uids: task.item.uids,
                  isPlayer: task.item.is_player,
                  onDelta: makeOnDelta(),
                }),
              () => refactorCancelRef.current,
            );
            if (result.character) characters.push(result.character);
            else failedTitles.push({ name, reason: "" });
          } else if (task.kind === "absorb") {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorRewriteOutcome>("refactor_absorb_entry", {
                  worldId: world,
                  entryUid: task.uid,
                  knownFields,
                  onDelta: makeOnDelta(),
                }),
              () => refactorCancelRef.current,
            );
            if (result.entry) refactorEntries.push(result.entry);
            else failedTitles.push({ name, reason: "" });
          } else if (task.kind === "group") {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorRewriteOutcome>("refactor_split_group", {
                  worldId: world,
                  groupId: task.group.id,
                  title: task.group.title,
                  kind: task.group.kind,
                  spans: task.group.spans,
                  knownFields,
                  onDelta: makeOnDelta(),
                }),
              () => refactorCancelRef.current,
            );
            if (result.entry) refactorEntries.push(result.entry);
            else failedTitles.push({ name, reason: "" });
          } else if (task.kind === "statusbar") {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorExpandOutcome>("refactor_expand_spans", {
                  worldId: world,
                  entryUid: task.uid,
                  spans: task.spans,
                  knownFields,
                  onDelta: makeOnDelta(),
                }),
              () => refactorCancelRef.current,
            );
            if (result.interface) interfaces.push(result.interface);
            else failedTitles.push({ name, reason: "" });
          } else {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorExpandOutcome>("refactor_expand", {
                  worldId: world,
                  entryUid: task.uid,
                  kind: survey.playable_interface_uids.includes(task.uid) ? "interface_shell" : "interface",
                  knownFields,
                  onDelta: makeOnDelta(),
                }),
              () => refactorCancelRef.current,
            );
            if (result.interface) interfaces.push(result.interface);
            else failedTitles.push({ name, reason: "" });
          }
        } catch (reason) {
          if (!String(reason).includes("refactor-aborted")) failedTitles.push({ name, reason: String(reason).slice(0, 200) });
        } finally {
          bumpDone();
        }
      };

      // chain 恆空：survey 同一 run 已建快取，warmed=true 跳過首發獨跑，pool 直接全並行開跑。
      await runRefactorCalls({
        chain: [],
        pool,
        limit: REFACTOR_PARALLEL_LIMIT,
        isCancelled: () => refactorCancelRef.current,
        run,
        warmed: true,
      });

      setRefactorProgress(null);
      if (characters.length > 0 || interfaces.length > 0 || refactorEntries.length > 0) {
        const outcome = assembleRefactorOutcome({
          characters,
          interfaces,
          entries: refactorEntries,
          dropped: local.dropped,
          unabsorbed: local.unabsorbed,
          audit: local.audit,
        });
        setRefactorOutcome(outcome);
        setRefactorSelection(defaultRefactorSelection(outcome));
        setRefactorOrigin("ai");
        setRefactorDetail(false);
      }
      setRefactorFailures(failedTitles);
    } catch (reason) {
      setRefactorProgress(null);
      if (String(reason).includes("refactor-aborted")) return;
      setWorldbookMessage(String(reason));
    }
  }

  // 取消：擋「還沒發的下一條」＋後端 abort 在途呼叫（refactor_abort，包 2 交付）——已經在燒
  // 的那幾條立刻中止，中止錯誤走 sentinel "refactor-aborted" 靜默略過，不列入失敗。
  function cancelAiRefactor() {
    refactorCancelRef.current = true;
    setRefactorProgress((current) => current && { ...current, cancelling: true });
    void invoke("refactor_abort", { worldId: world });
  }

  // AI 卡重構：零額度測試用入口——直接餵一份產物 JSON，跳過真 AI 呼叫，驗證人審／套用路徑用。
  async function pickRefactorOutcome(file: File) {
    setWorldbookMessage("");
    try {
      const outcome = parseRefactorOutcome(await file.text());
      setRefactorOutcome(outcome);
      setRefactorSelection(defaultRefactorSelection(outcome));
      setRefactorOrigin("import");
      setRefactorDetail(false);
    } catch (reason) {
      const invalid = reason instanceof Error && reason.message === REFACTOR_IMPORT_INVALID;
      setWorldbookMessage(invalid ? t("refactorImportInvalid") : String(reason));
    }
  }

  function closeRefactor() {
    setRefactorOutcome(null);
    setRefactorSelection(null);
    setRefactorOrigin(null);
    setRefactorDetail(false);
    setRefactorFailures([]);
  }

  // 已淘汰清單的「放回」：零後端行為，走既有 entries 勾選路徑——套用時跟其他世界書條目一視同仁。
  function restoreDroppedItem(index: number) {
    if (!refactorOutcome || !refactorSelection) return;
    const result = restoreDropped(refactorOutcome, refactorSelection, index);
    setRefactorOutcome(result.outcome);
    setRefactorSelection(result.selection);
  }

  function refactorApplyMessage(summary: RefactorApplySummary) {
    return [
      summary.new_characters > 0 && t("refactorApplyDoneCharacters", { n: summary.new_characters }),
      summary.player_assigned && t("refactorApplyDonePlayer"),
      summary.new_entries > 0 && t("refactorApplyDoneEntries", { n: summary.new_entries }),
      summary.deleted_entries > 0 && t("refactorApplyDoneDeleted", { n: summary.deleted_entries }),
      summary.interface_applied && t("refactorApplyDoneInterface"),
      summary.mechanisms_applied > 0 && t("refactorApplyDoneMechanisms", { n: summary.mechanisms_applied }),
    ]
      .filter(Boolean)
      .join("・");
  }

  async function applyRefactor(selection: RefactorSelection) {
    if (!refactorOutcome || refactorBusy) return;
    setWorldbookMessage("");
    setRefactorBusy(true);
    try {
      const summary = await invoke<RefactorApplySummary>("refactor_apply", {
        worldId: world,
        outcome: refactorOutcome,
        selection,
        recordReceipt: refactorOrigin !== "ai",
      });
      closeRefactor();
      await refreshWorldbook();
      await refreshLedger();
      await refreshCast();
      await onRefactorApplied();
      await showMessage(refactorApplyMessage(summary), { title: t("refactorBtn") });
    } catch (reason) {
      setWorldbookMessage(String(reason));
    } finally {
      setRefactorBusy(false);
    }
  }

  // 匯出結果卡上這份還沒套用（或剛套用完）的產物，供之後用「匯入重構卡」讀回重玩。
  async function exportRefactorOutcome() {
    if (!refactorOutcome || refactorBusy) return;
    setWorldbookMessage("");
    try {
      const path = await saveDialog({
        defaultPath: refactorCardFileName(worldName),
        filters: [{ name: t("refactorOutcomeJson"), extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("refactor_export_outcome", { outcome: refactorOutcome, path });
      await revealItemInDir(path);
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  // 手動「轉成角色卡」一律轉一般卡——玩家卡另有從頭建立的入口，AI 卡重構的勾選畫面也能指定
  // 玩家卡（要點 4），這顆按鈕不再問「要不要轉成玩家卡」。
  async function convertEntryToCharacter() {
    if (!draft || draft.uid === null) return;
    setWorldbookMessage("");
    try {
      const meta = await invoke<CharacterMeta>("worldbook_entry_to_character", {
        worldId: world,
        uid: draft.uid,
        color: convertColor,
        asPlayer: false,
      });
      setDraft(null);
      await refreshWorldbook();
      setWorldbookMessage(t("convertEntryDone", { name: meta.name }));
      await onEntryConverted();
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  // 條目表單就地展開：編輯取代原本那一列、新增排在清單底部（2026-07-30 使用者回饋——
  // 表單固定在頂端時，點下方條目的編輯完全看不出反應）。按鈕照全 app 慣例置頂。
  const entryForm = draft && (
    <form ref={draftFormRef} className="settings-form worldbook-form" onSubmit={saveEntry}>
      <div className="row">
        <button type="submit">{t("worldbookSaveEntry")}</button>
        <button type="button" onClick={() => void closeDraft()}>
          {t("worldbookCancel")}
        </button>
        {draft.uid !== null && (
          <button type="button" onClick={() => void convertEntryToCharacter()}>
            {t("convertEntryToCard")}
          </button>
        )}
      </div>
      <label>
        {t("worldbookEntryTitle")}
        <input
          value={draft.title}
          onChange={(event) => setDraft({ ...draft, title: event.currentTarget.value })}
        />
      </label>
      <label>
        {t("worldbookKeys")}
        <input
          value={draft.keys}
          placeholder={t("worldbookKeysHint")}
          onChange={(event) => setDraft({ ...draft, keys: event.currentTarget.value })}
        />
      </label>
      <label>
        {t("worldbookContent")}
        <textarea
          rows={7}
          value={draft.content}
          onChange={(event) => setDraft({ ...draft, content: event.currentTarget.value })}
        />
      </label>
      <label className="inline">
        <input
          type="checkbox"
          checked={draft.constant}
          onChange={(event) => setDraft({ ...draft, constant: event.currentTarget.checked })}
        />
        {t("worldbookConstantLabel")}
      </label>
      <label className="inline">
        <input
          type="checkbox"
          checked={draft.enabled}
          onChange={(event) => setDraft({ ...draft, enabled: event.currentTarget.checked })}
        />
        {t("worldbookEnabled")}
      </label>
      <fieldset className="worldbook-visibility">
        <legend>{t("worldbookVisibility")}</legend>
        {(["gm", "public", "characters"] as const).map((visibility) => (
          <label className="inline" key={visibility}>
            <input
              type="radio"
              name="worldbook-visibility"
              value={visibility}
              checked={draft.visibility === visibility}
              onChange={() => {
                setDraft({ ...draft, visibility });
                // 點「指定角色」當下重抓在場角色：畫面開著時可能剛從隱藏區還原角色
                if (visibility === "characters") {
                  void invoke<CharacterMeta[]>("list_characters", { worldId: world }).then((cast) =>
                    setCharacters(cast.filter((character) => !character.archived)),
                  );
                }
              }}
            />
            {visibility === "gm"
              ? t("worldbookVisibilityGm")
              : visibility === "public"
                ? t("worldbookVisibilityPublic")
                : t("worldbookVisibilityCharacters")}
          </label>
        ))}
      </fieldset>
      {draft.visibility === "characters" && (
        <fieldset className="worldbook-characters">
          <legend>{t("worldbookChooseCharacters")}</legend>
          {characters.length === 0 ? (
            <span>{t("worldbookNoCharacters")}</span>
          ) : (
            characters.map((character) => (
              <label className="inline" key={character.id}>
                <input
                  type="checkbox"
                  checked={draft.characters.includes(character.id)}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      characters: event.currentTarget.checked
                        ? [...draft.characters, character.id]
                        : draft.characters.filter((id) => id !== character.id),
                    })
                  }
                />
                {character.name}
              </label>
            ))
          )}
        </fieldset>
      )}
    </form>
  );

  // 結果卡摘要行只列有產物的區：「拆出 N 個角色」「介面」「收編 N 條規則」以「・」串接
  const refactorCounts = refactorOutcome ? refactorSummaryCounts(refactorOutcome) : null;
  const refactorSummaryParts = refactorCounts
    ? [
        refactorCounts.characters > 0 && t("refactorSummaryCharacters", { n: refactorCounts.characters }),
        refactorCounts.hasInterface && t("refactorSummaryInterface"),
        refactorCounts.entries > 0 && t("refactorSummaryEntries", { n: refactorCounts.entries }),
        refactorCounts.mechanisms > 0 && t("refactorSummaryMechanisms", { n: refactorCounts.mechanisms }),
      ].filter((part): part is string => Boolean(part))
    : [];

  return (
    <>
      <form onSubmit={saveWorldSettings} className="settings-form">
        {/* 按鈕列放文字框上方：長文編輯時儲存／返回固定在最顯眼處（2026-07-24 使用者回饋） */}
        <div className="row">
          <button type="submit">{t("saveWorld")}</button>
          <button type="button" onClick={() => void handleBack()}>
            {t("backToNow")}
          </button>
          {message && <span>{message}</span>}
          {unsavedCount > 0 && (
            <span className="unsaved-hint" role="status">
              {t("unsavedChanges", { n: unsavedCount })}
            </span>
          )}
        </div>
        <textarea
          rows={6}
          aria-label={t("worldAria")}
          value={text}
          onChange={(e) => setText(e.currentTarget.value)}
        />
      </form>

      <section className="worldbook-section" aria-labelledby="worldbook-title">
        <h3 id="worldbook-title">{t("worldbookTitle")}</h3>
        <div className="worldbook-actions">
          <button type="button" onClick={addEntry}>
            {t("worldbookAddEntry")}
          </button>
          <button type="button" onClick={() => void dedupeWorldbook()}>
            {t("worldbookDedupe")}
          </button>
          <button type="button" onClick={() => void exportWorldbook()}>
            {t("worldbookExport")}
          </button>
          <button
            type="button"
            className="ai-gen-btn"
            title={t("refactorBtnHint")}
            disabled={refactorProgress !== null}
            onClick={() => void runAiRefactor()}
          >
            ✨ {t("refactorBtn")}
          </button>
          <button
            type="button"
            title={t("refactorImportBtnHint")}
            disabled={refactorProgress !== null}
            onClick={() => refactorInputRef.current?.click()}
          >
            {t("refactorImportBtn")}
          </button>
          <button
            type="button"
            disabled={refactorProgress !== null}
            onClick={() => void exportSavedRefactorOutcome()}
          >
            {t("refactorExportSavedBtn")}
          </button>
          <input
            ref={refactorInputRef}
            type="file"
            accept=".json,application/json"
            hidden
            onChange={(e) => {
              const file = e.currentTarget.files?.[0];
              e.currentTarget.value = "";
              if (file) void pickRefactorOutcome(file);
            }}
          />
        </div>

        {/* 標準流程零必看：只有真的有東西被接管／跳過，或有記帳次數時才出現這塊。 */}
        {(ledger.entries.length > 0 ||
          ledger.rejected > 0 ||
          ledger.clamped > 0 ||
          ledger.errors > 0 ||
          ledger.jumps > 0) && (
          <details className="mechanism-ledger">
            <summary>{t("ledgerTitle")}</summary>
            {ledger.entries.length > 0 && (
              <div className="mechanism-ledger-list">
                {ledger.entries.map((entry) => (
                  <div className="mechanism-ledger-row" key={entry.uid}>
                    <div className="mechanism-ledger-summary">
                      <strong>{entry.title}</strong>
                      <span className="worldbook-badge">
                        {entry.kind === "absorbed" ? t("ledgerAbsorbed") : t("ledgerSkipped")}
                      </span>
                      <span className="mechanism-ledger-detail">{entry.detail}</span>
                    </div>
                    {!entries.find((worldbookEntry) => worldbookEntry.uid === entry.uid)?.locked && (
                    <label className="mechanism-ledger-toggle">
                      <input
                        type="checkbox"
                        checked={entry.sent}
                        onChange={() => void toggleLedgerEntry(entry)}
                      />
                      {t("ledgerSendRaw")}
                    </label>
                    )}
                  </div>
                ))}
              </div>
            )}
            {(ledger.rejected > 0 || ledger.clamped > 0 || ledger.errors > 0 || ledger.jumps > 0) && (
              <p className="mechanism-ledger-stats">
                {[
                  ledger.rejected > 0 && t("ledgerStatsRejected", { n: ledger.rejected }),
                  ledger.clamped > 0 && t("ledgerStatsClamped", { n: ledger.clamped }),
                  ledger.errors > 0 && t("ledgerStatsErrors", { n: ledger.errors }),
                  ledger.jumps > 0 && t("ledgerStatsJumps", { n: ledger.jumps }),
                ]
                  .filter(Boolean)
                  .join("　")}
              </p>
            )}
          </details>
        )}

        {entries.length === 0 ? (
          <p className="worldbook-empty">{t("worldbookEmpty")}</p>
        ) : (
          <div className="worldbook-list">
            {entryDrag.order.map((entry) =>
              draft && draft.uid === entry.uid ? (
                <Fragment key={entry.uid}>{entryForm}</Fragment>
              ) : (
              <div
                className={`worldbook-row${entry.disabled ? " worldbook-row-disabled" : ""}${
                  entryDrag.draggingKey === String(entry.uid) ? " row-dragging" : ""
                }`}
                key={entry.uid}
                title={t("dragToReorder")}
                {...entryDrag.rowProps(entry)}
              >
                <div className="worldbook-summary">
                  {(() => {
                    const head = (
                      <>
                        <strong>{entry.title || entry.uid}</strong>
                        <span>{entry.keys.join("、") || t("worldbookNoKeys")}</span>
                        <div className="worldbook-badges">
                          {entry.constant && (
                            <span className="worldbook-badge">{t("worldbookConstant")}</span>
                          )}
                          {/* 可見範圍＝資訊邊界：全 app 統一的虛線琥珀機密記號 */}
                          <span className="worldbook-badge worldbook-badge-visibility">
                            {entry.visibility.type === "gm"
                              ? t("worldbookVisibilityGm")
                              : entry.visibility.type === "public"
                                ? t("worldbookVisibilityPublic")
                                : t("worldbookCharacterCount", {
                                    n: entry.visibility.characters.length,
                                  })}
                          </span>
                          {entry.disabled && (
                            <span className="worldbook-badge">{t("worldbookDisabled")}</span>
                          )}
                          {entry.locked && <span className="worldbook-badge">🔒 {t("worldbookLocked")}</span>}
                        </div>
                      </>
                    );
                    // 鎖定條目不能編輯，但說明文的家就在世界書：標題列可展開唯讀全文。
                    return entry.locked ? (
                      <details className="worldbook-locked-view">
                        <summary>{head}</summary>
                        <div className="worldbook-locked-content">{entry.content}</div>
                      </details>
                    ) : (
                      head
                    );
                  })()}
                </div>
                {!entry.locked && <div className="worldbook-row-actions">
                  <button type="button" onClick={() => editEntry(entry)}>
                    {t("editBtn")}
                  </button>
                  <button type="button" onClick={() => void deleteEntry(entry)}>
                    {t("worldbookDelete")}
                  </button>
                </div>
                }
              </div>
              ),
            )}
          </div>
        )}
        {draft && draft.uid === null && entryForm}
        {worldbookMessage && <p role="status">{worldbookMessage}</p>}
      </section>

      {refactorProgress && (
        <div className="modal-overlay">
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("refactorBtn")}>
            <h2>{t("refactorBtn")}</h2>
            <p role="status">{refactorProgress.text}</p>
            {refactorProgress.tail && <pre className="refactor-stream-tail">{refactorProgress.tail}</pre>}
            <div className="ai-gen-footer">
              <button type="button" disabled={refactorProgress.cancelling} onClick={cancelAiRefactor}>
                {t("refactorCancel")}
              </button>
            </div>
          </div>
        </div>
      )}

      {refactorOutcome && refactorSelection && (
        // 點視窗外不關閉（2026-08-12 拍板）：誤觸一下整份重構結果就丟了，關閉只走「不要」鍵
        <div className="modal-overlay">
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label={t("refactorResultTitle")}
          >
            <h2>{refactorFailures.length > 0 ? t("refactorResultPartialTitle") : t("refactorResultTitle")}</h2>
            {refactorFailures.length > 0 && (
              <p className="usage-bad" role="alert">
                {t("refactorPartialFailed", { n: refactorFailures.length, names: refactorFailures.map((f) => f.name).join("、") })}
                {[...new Set(refactorFailures.map((f) => f.reason).filter(Boolean))].map((reason) => (
                  <span key={reason} className="refactor-fail-reason">
                    {t("refactorFailReason", { reason })}
                  </span>
                ))}
              </p>
            )}
            {!refactorDetail ? (
              <>
                {refactorSummaryParts.length > 0 && <p>{refactorSummaryParts.join("・")}</p>}
                <div className="ai-gen-footer">
                  <button type="button" disabled={refactorBusy} onClick={closeRefactor}>
                    {t("refactorDismiss")}
                  </button>
                  <button type="button" disabled={refactorBusy} onClick={() => void exportRefactorOutcome()}>
                    {t("refactorExportBtn")}
                  </button>
                  <button type="button" disabled={refactorBusy} onClick={() => setRefactorDetail(true)}>
                    {t("refactorExpand")}
                  </button>
                  <button
                    type="button"
                    className="ai-gen-submit"
                    disabled={refactorBusy}
                    onClick={() => void applyRefactor(defaultRefactorSelection(refactorOutcome))}
                  >
                    {t("refactorApplyAll")}
                  </button>
                </div>
              </>
            ) : (
              <>
                {refactorOutcome.characters.length > 0 && (
                  <section>
                    <h3>{t("refactorSectionCharacters")}</h3>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.characters.map((character, index) => (
                        <div className="mechanism-ledger-row" key={index}>
                          <details>
                            <summary>
                              <label className="inline" onClick={(event) => event.stopPropagation()}>
                            <input
                              type="checkbox"
                              checked={refactorSelection.character_indices.includes(index)}
                              onClick={(event) => event.stopPropagation()}
                              onChange={(event) => {
                                const checked = event.currentTarget.checked;
                                setRefactorSelection(
                                  (selection) =>
                                    selection &&
                                    (checked
                                      ? { ...selection, character_indices: toggleIndex(selection.character_indices, index, true) }
                                      : unselectCharacter(selection, index)),
                                );
                              }}
                            />
                            {character.emoji} {character.name}
                          </label>
                            </summary>
                            <span className="refactor-source">{t("refactorSourceLabel", { titles: sourceEntryTitles(entries, character.source_uids) })}</span>
                            <p>{t("refactorCharPublic")}</p>
                            <div style={{ whiteSpace: "pre-wrap" }}>{character.public_md}</div>
                            <p>{t("refactorCharPrivate")}</p>
                            <div style={{ whiteSpace: "pre-wrap" }}>{character.private_md}</div>
                          </details>
                          {/* 玩家卡只問 AI 認定是 {{user}} 的那一位：多數卡都預設好玩家是誰，
                              讓任意角色都能被選成玩家卡不符合卡的設計。 */}
                          {character.suspected_player && (
                            <label className="mechanism-ledger-toggle">
                              <input
                                type="checkbox"
                                checked={refactorSelection.player_index === index}
                                onChange={(event) =>
                                  setRefactorSelection(
                                    (selection) =>
                                      selection && setPlayerIndex(selection, event.currentTarget.checked ? index : null),
                                  )
                                }
                              />
                              {t("refactorPlayerCheckLabel")}
                            </label>
                          )}
                        </div>
                      ))}
                    </div>
                  </section>
                )}
                {refactorOutcome.entries.length > 0 && (
                  <section>
                    <h3>{t("refactorSectionEntries")}</h3>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.entries.map((entry, index) => (
                        <div className="mechanism-ledger-row" key={index}>
                          <details>
                            <summary>
                              <label className="inline" onClick={(event) => event.stopPropagation()}>
                                <input
                                  type="checkbox"
                                  checked={refactorSelection.entry_indices.includes(index)}
                                  onClick={(event) => event.stopPropagation()}
                                  onChange={(event) => {
                                    const checked = event.currentTarget.checked;
                                    setRefactorSelection((selection) => selection && {
                                      ...selection,
                                      entry_indices: toggleIndex(selection.entry_indices, index, checked),
                                    });
                                  }}
                                />
                                {entry.title}
                              </label>
                              <span className="worldbook-badge">
                                {entry.kind === "setting" ? t("refactorEntryKindSetting") : t("refactorEntryKindMechanism")}
                              </span>
                              {entry.kind === "mechanism" && (Object.keys(entry.rules ?? {}).length > 0 || (entry.triggers?.length ?? 0) > 0) && (
                                <span className="worldbook-badge">🔒 {t("worldbookLocked")}</span>
                              )}
                            </summary>
                            <span className="refactor-source">{t("refactorSourceLabel", { titles: sourceEntryTitles(entries, entry.source_uids) })}</span>
                            <div style={{ whiteSpace: "pre-wrap" }}>{entry.content}</div>
                          </details>
                        </div>
                      ))}
                    </div>
                  </section>
                )}
                {refactorOutcome.interface && (
                  <section>
                    <h3>{t("refactorSectionInterface")}</h3>
                    <div className="mechanism-ledger-list">
                      <details>
                        <summary>
                        <label className="inline" onClick={(event) => event.stopPropagation()}>
                        <input
                          type="checkbox"
                          checked={refactorSelection.apply_interface}
                          onClick={(event) => event.stopPropagation()}
                          onChange={(event) => {
                            const checked = event.currentTarget.checked;
                            setRefactorSelection((selection) => selection && { ...selection, apply_interface: checked });
                          }}
                        />
                        {t("refactorSummaryInterface")}
                      </label>
                        </summary>
                        <span className="refactor-source">
                          {t("refactorSourceLabel", { titles: sourceEntryTitles(entries, refactorOutcome.interface.source_uids) })}
                        </span>
                        <div>{t("refactorInterfaceFields", { names: typeof refactorOutcome.interface.state_fields === "object" && refactorOutcome.interface.state_fields !== null && !Array.isArray(refactorOutcome.interface.state_fields) ? Object.keys(refactorOutcome.interface.state_fields).join("、") : "" })}</div>
                      </details>
                    </div>
                  </section>
                )}
                {refactorOutcome.mechanisms.length > 0 && (
                  <section>
                    <h3>{t("refactorSectionMechanisms")}</h3>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.mechanisms.map((mechanism, index) => (
                        <label className="inline" key={index}>
                          <input
                            type="checkbox"
                            checked={refactorSelection.mechanism_indices.includes(index)}
                            onChange={(event) => {
                              const checked = event.currentTarget.checked;
                              setRefactorSelection(
                                (selection) =>
                                  selection && {
                                    ...selection,
                                    mechanism_indices: toggleIndex(selection.mechanism_indices, index, checked),
                                  },
                              );
                            }}
                          />
                          {sourceEntryTitle(entries, mechanism.source_uid)}
                        </label>
                      ))}
                    </div>
                  </section>
                )}
                {/* 已淘汰：判官整條／半條丟棄的內容快照，預設收起——玩家想確認才展開，救回來就是
                    普通世界書條目，走下面既有的套用路徑，沒有新的後端行為。 */}
                {refactorOutcome.dropped.length > 0 && (
                  <details className="mechanism-ledger">
                    <summary>{t("refactorDroppedSection", { n: refactorOutcome.dropped.length })}</summary>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.dropped.map((item, index) => (
                        <div className="mechanism-ledger-row" key={index}>
                          <details>
                            <summary>
                              {item.title}{" "}
                              <span className="worldbook-badge">
                                {t(REFACTOR_DROPPED_RULE_KEYS[item.rule] ?? "refactorDroppedRule1")}
                              </span>
                            </summary>
                            <div style={{ whiteSpace: "pre-wrap" }}>{item.content}</div>
                          </details>
                          <button type="button" onClick={() => restoreDroppedItem(index)}>
                            {t("refactorDroppedRestore")}
                          </button>
                        </div>
                      ))}
                    </div>
                  </details>
                )}
                {/* 未接管機制：純資訊，原文已經照搬進 GM 規則條目——不會遺失，只是還沒有系統畫面。 */}
                {refactorOutcome.unabsorbed.length > 0 && (
                  <section>
                    <h3>{t("refactorUnabsorbedSection", { n: refactorOutcome.unabsorbed.length })}</h3>
                    <p className="mechanism-ledger-detail">{t("refactorUnabsorbedHint")}</p>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.unabsorbed.map((item, index) => (
                        <div className="mechanism-ledger-row" key={index}>
                          <div className="mechanism-ledger-summary">
                            <strong>{item.title}</strong>
                            <span className="mechanism-ledger-detail">{item.note}</span>
                            <span className="refactor-source">{item.span || item.uid}</span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </section>
                )}
                {/* 稽核：機械檢查抓到的紅字，純資訊不影響套用——detail 是後端已經寫好的繁中一句。 */}
                {refactorOutcome.audit.length > 0 && (
                  <section>
                    <h3>{t("refactorAuditSection")}</h3>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.audit.map((item, index) => (
                        <div className="mechanism-ledger-row" key={index}>
                          <div className="mechanism-ledger-summary">
                            <span className="worldbook-badge">
                              {t(REFACTOR_AUDIT_KIND_KEYS[item.kind] ?? "refactorAuditKindCoverage")}
                            </span>
                            <span className="refactor-source">{item.span || item.uid}</span>
                            <span className={item.kind === "excused" ? "mechanism-ledger-detail" : "usage-bad"}>
                              {item.detail}
                            </span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </section>
                )}
                <div className="ai-gen-footer">
                  <button type="button" disabled={refactorBusy} onClick={() => setRefactorDetail(false)}>
                    {t("settingsBack")}
                  </button>
                  <button
                    type="button"
                    className="ai-gen-submit"
                    disabled={refactorBusy}
                    onClick={() => void applyRefactor(refactorSelection)}
                  >
                    {t("refactorApplyBtn")}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </>
  );
}

// 卡片／世界設定編輯共用整面外框：與單幕閱讀同款（頂部標題，下方內容填滿），不是 modal——
// 使用者拍板：主欄下半部（messages＋composer）整面取代，composer 不渲染＝編輯中無法發言。
// 「返回」不在這裡：使用者拍板放在表單的儲存鈕旁邊，由 CardEditor／WorldEditor 自己渲染
function EditPane({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <>
      <div className="act-reader-header">
        <strong>{title}</strong>
      </div>
      <div className="edit-pane-body">{children}</div>
    </>
  );
}

// 單幕閱讀：整面取代對話畫面（不是 modal），頂部一行標題＋匯出＋返回，下方唯讀事件列表填滿到底
function ActReader({
  world,
  worldName,
  scene,
  label,
  onBack,
  onFork,
}: {
  world: string;
  worldName: string;
  scene: number;
  label: string;
  onBack: () => void;
  onFork: () => void;
}) {
  const [events, setEvents] = useState<TranscriptEvent[] | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    setEvents(null);
    setError("");
    invoke<TranscriptEvent[]>("read_transcript", { worldId: world, scene })
      .then(setEvents)
      .catch((reason) => setError(String(reason)));
  }, [world, scene]);

  async function exportScene() {
    setError("");
    try {
      const now = new Date();
      const pad = (n: number) => String(n).padStart(2, "0");
      const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}${pad(now.getMinutes())}`;
      const path = await saveDialog({
        defaultPath: `${t("sceneExportFileName", { table: worldName, n: scene + 1, stamp })}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      await invoke("export_scene", { worldId: world, scene, path });
      await revealItemInDir(path);
    } catch (reason) {
      setError(String(reason));
    }
  }

  return (
    <>
      <div className="act-reader-header">
        <strong>{label}</strong>
        <button type="button" onClick={exportScene}>
          {t("exportScene")}
        </button>
        <button type="button" onClick={onBack}>
          {t("backToNow")}
        </button>
        {/* 分岔續玩：整面畫面唯一往前推進的動作，靠右與唯讀那幾顆分開 */}
        <button type="button" className="act-fork" onClick={onFork}>
          {t("sceneFork")}
        </button>
      </div>
      <section className="messages" aria-label={label}>
        {events === null ? (
          error && <ErrorNote text={error} />
        ) : (
          events.map((event, index) => (
            <div key={index} className={`scene-event scene-event-${event.kind}`}>
              {(event.kind === "dialogue" || event.kind === "player") && (
                <span className="speaker">{event.speaker_name}</span>
              )}
              <StoryText text={event.text} />
            </div>
          ))
        )}
      </section>
      {error && events !== null && <ErrorNote text={error} />}
    </>
  );
}

function App() {
  const [worlds, setWorlds] = useState<WorldMeta[]>([]);
  // table 存桌 id；顯示名一律經 tableName（見下）從 worlds 查
  const [table, setTable] = useState("");
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [sponsorUnlocked, setSponsorUnlocked] = useState(false);
  const [characters, setCharacters] = useState<CharacterMeta[]>([]);
  const [playerCard, setPlayerCard] = useState<CharacterCard | null>(null);
  // 分支指認清單：每條狀態樹分支目前綁給哪個角色，換桌／讀狀態一起重載
  const [branchBindings, setBranchBindings] = useState<BranchBinding[]>([]);
  // 本幕出場集合：換桌／切幕由 enterTable 呼叫 scene_appearances 初始化，
  // 之後每次 gm_narrate 回傳的 arrived_characters 併入——auto_hidden 卡一登場就立刻從隱藏區移回主區
  const [sceneAppearances, setSceneAppearances] = useState<Set<string>>(new Set());
  const activeCharacters = characters.filter((character) => !isCharacterHidden(character, sceneAppearances));
  const archivedCharacters = characters.filter((character) => isCharacterHidden(character, sceneAppearances));
  const castDrag = useDragReorder(
    activeCharacters,
    (character) => character.id,
    (ordered) => void reorderCast(ordered),
  );
  // 角色卡編輯器每次 render 掛上「可以離開嗎」；側欄任何會換掉編輯畫面的入口都先問它
  const leaveGuard = useRef<(() => Promise<boolean>) | null>(null);
  // 角色圖快取：角色 id → data URL（來源是匯入時存下的原 PNG，後端 read_character_image）
  const [characterImages, setCharacterImages] = useState<Record<string, string>>({});
  const [characterAvatars, setCharacterAvatars] = useState<Record<string, string>>({});
  const [playerImage, setPlayerImage] = useState<string | null>(null);
  const [playerAvatar, setPlayerAvatar] = useState<string | null>(null);
  // GM 卡的圖：世界書匯入的是 PNG 卡時後端存下的那張，null＝回退內建書本圖
  const [gmImage, setGmImage] = useState<string | null>(null);
  const [speaker, setSpeaker] = useState("");
  const [scene, setScene] = useState(0);
  const [sceneTitles, setSceneTitles] = useState<Record<string, string>>({});
  const [sceneLabels, setSceneLabels] = useState<Record<string, SceneLabel>>({});
  const [tableState, setTableState] = useState<Record<string, string>>({});
  const [tableTree, setTableTree] = useState<Record<string, StateNode>>({});
  const [tableJumps, setTableJumps] = useState<Record<string, string>>({});
  const [events, setEvents] = useState<TranscriptEvent[]>([]);
  // 這一輪收回的那幾句，後收的疊在最上面（復原一次拿一則，順序自然還原）。
  // 記下當時的桌與幕，換桌換幕後整疊自動失效（比對不上就不顯示），免得放回錯的地方
  const [undone, setUndone] = useState<{
    table: string;
    scene: number;
    events: TranscriptEvent[];
  } | null>(null);
  // 連按時前一次的寫檔還沒回來就再按，兩次會讀到同一份舊狀態而重複收回／放回同一則；
  // 用旗標讓同一時間只跑一次（寫檔是毫秒級，擋掉的那下感覺不出來）
  const undoBusy = useRef(false);
  // Enter 送出會接著觸發 blur，Esc 也會先失焦；旗標避免重複送出或把取消誤存。
  const stateFieldSaveBusy = useRef(false);
  const stateFieldEditCancelled = useRef(false);
  const [input, setInput] = useState("");
  // 逐角色打字指示：狀態帶「是誰在生成、以哪種形式」，不做全域單一指示燈（NewPlan §9.2）
  // id 空字串＝GM（narration 一律如此，dialogue 一定帶角色 id）；顯示名經 metaOf(id) 即時查
  const [generating, setGenerating] = useState<{
    id: string;
    kind: "dialogue" | "narration";
  } | null>(null);
  const [streamText, setStreamText] = useState("");
  // 保溫 ping 的節奏狀態：上次真正推進的時刻、已連發幾次、這桌是否根本沒得保溫（非 claude 模式）
  const generatingRef = useRef<{ id: string; kind: "dialogue" | "narration" } | null>(null);
  generatingRef.current = generating;
  const lastTurnAt = useRef(Date.now());
  const pingCount = useRef(0);
  const keepaliveOff = useRef(false);
  const [awayTooLong, setAwayTooLong] = useState(false);
  // 改桌名可從兩處進入：主欄標題（header）與側欄目前桌那一列（list）；at 決定輸入框長在哪
  const [editingName, setEditingName] = useState<{
    at: "header" | "list";
    value: string;
  } | null>(null);
  // false＝關閉；字串＝開啟並落在該分頁（生圖對話框的「AI 連線設定」鈕直開 ai 分頁）
  const [settingsOpen, setSettingsOpen] = useState<false | "appearance" | "ai">(false);
  // 主欄下半部（messages＋composer）三選一整面取代：單幕閱讀／角色卡編輯／GM 世界設定編輯
  // （使用者拍板改版：需求 4 不用 modal，與需求 3 單幕閱讀同一套「整面取代」模式）
  const [mainView, setMainView] = useState<
    | { kind: "scene"; n: number }
    | { kind: "character"; id: string }
    | { kind: "new-character"; id: string }
    | { kind: "player"; id: string }
    | { kind: "new-player"; id: string }
    | { kind: "world" }
    | null
  >(null);
  // 四種卡片編輯畫面都帶 id，先收斂成一個值，下面就只需要問「是不是玩家卡」
  const cardView =
    mainView?.kind === "character" ||
    mainView?.kind === "new-character" ||
    mainView?.kind === "player" ||
    mainView?.kind === "new-player"
      ? mainView
      : null;
  const editingPlayerCard = cardView?.kind === "player" || cardView?.kind === "new-player";
  // 側欄描邊＝側欄當下選中的那張：編輯畫面時是正在編輯的卡，其餘畫面是發言對象（編輯不動發言對象）
  const selectedCard =
    mainView?.kind === "character"
      ? mainView.id
      : mainView?.kind === "new-character"
        ? ""
        : speaker;
  // 前幕清單浮層：只是開關狀態，不佔版面高度（NewPlan §9.4 主欄閱讀優先改造）
  const [actsOpen, setActsOpen] = useState(false);
  // 設定頁改語言後問一次「範例桌要不要換語言重生」；值＝改之前的語言，取消時用來回退
  const [regenAsk, setRegenAsk] = useState<Lang | null>(null);
  const [openingChoice, setOpeningChoice] = useState<string[] | null>(null);
  // 一次只展開一條：面板不長，攤開多條反而找不到自己在看哪一段
  const [openingExpanded, setOpeningExpanded] = useState<number | null>(null);
  // 開場白翻譯：逐則狀態＋「全部翻譯」是否在跑；abort ref 給 modal 一關就停止後續翻譯呼叫用
  // （純 ref 而非 state：序列迴圈中途要讀到「使用者剛剛關掉視窗」，不能等下一次 render）
  const [openingTransState, setOpeningTransState] = useState<Record<number, "translating" | "done" | "error">>({});
  const [openingTransAllBusy, setOpeningTransAllBusy] = useState(false);
  const openingTransAbort = useRef(false);
  // openingChoice 一變成 null（不管哪個按鈕關的）就中止：不必在每個關閉入口各補一次旗標
  useEffect(() => {
    if (openingChoice === null) openingTransAbort.current = true;
  }, [openingChoice]);
  // 匯入身分框：等玩家在三鍵框挑一種，data 原樣留著給兩條路徑共用；
  // booksFirst＝主按鈕指世界書（探測結果只用來算這個，算完就不必留著）
  const [importChoice, setImportChoice] = useState<{ data: number[]; name: string; booksFirst: boolean } | null>(
    null,
  );
  // 第二張卡路由框：身分已定、桌上已有匯入紀錄才會跳出來；ask＝一般第二張卡、merge_worldbook＝第二本世界書會合成一本
  const [importRoute, setImportRoute] = useState<{
    data: number[];
    identity: "character" | "worldbook";
    label: string;
    route: "ask" | "merge_worldbook";
  } | null>(null);
  const [error, setError] = useState("");
  const [sidebarWidth, setSidebarWidth] = useState(
    () => Number(localStorage.getItem(SIDEBAR_WIDTH_KEY)) || SIDEBAR_DEFAULT_WIDTH,
  );
  const [tableListOpen, setTableListOpen] = useState(
    () => localStorage.getItem(TABLE_LIST_OPEN_KEY) !== "false",
  );
  const [stateBarOpen, setStateBarOpen] = useState(
    () => localStorage.getItem(STATE_BAR_OPEN_KEY) === "true",
  );
  // 狀態列只給有匯入狀態列規則的桌：其他桌整條不掛上去，也就打不開
  const [hasStateBar, setHasStateBar] = useState(false);
  // 這桌各卡的介面腳本（DRM／雲端載入器卡沒有腳本，不進這份清單）；面板是選配功能，讀失敗就當沒有
  const [cardInterfaces, setCardInterfaces] = useState<CardInterface[]>([]);
  // AI 重構套用介面規則時可能順便產的靜態渲染殼；沒重構過或那次沒產殼就是 null，退回卡片自帶殼／event.raw 找殼
  const [refactorShell, setRefactorShell] = useState<string | null>(null);
  const [cardUiOpen, setCardUiOpen] = useState(false);
  // 這桌的匯入收據摘要：非空、且還沒開始跟 AI 對話，才顯示「復原上次匯入」按鈕
  const [importReceipts, setImportReceipts] = useState<ImportReceiptSummary[]>([]);
  const [chattedSinceImport, setChattedSinceImport] = useState(false);
  // 復原動作可能改動世界書／機制資料；世界設定畫面若剛好開著就靠改這把 key 強制整個重新掛載重載
  const [worldEditorRefreshKey, setWorldEditorRefreshKey] = useState(0);
  // 編輯中的欄位：path 是樹裡的完整路徑，平欄則是長度 1 的路徑（tree=false，走舊的單層存檔）
  const [editingStateField, setEditingStateField] = useState<{
    path: string[];
    tree: boolean;
    value: string;
  } | null>(null);
  const [genTableOpen, setGenTableOpen] = useState(false);
  const [genInput, setGenInput] = useState("");
  const [genGenres, setGenGenres] = useState<string[]>([]);
  const [genOutline, setGenOutline] = useState<GeneratedOutline | null>(null);
  const [genOutlineRaw, setGenOutlineRaw] = useState<string | null>(null);
  const [genResultRaw, setGenResultRaw] = useState<string | null>(null);
  const [genResultMessage, setGenResultMessage] = useState<"outline" | "character">("outline");
  const [genError, setGenError] = useState("");
  const [genBusy, setGenBusy] = useState<"outline" | "character" | "expand" | null>(null);
  const [genCharacterHint, setGenCharacterHint] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);
  const importInputRef = useRef<HTMLInputElement>(null);

  async function loadCharacterImages(worldId: string, cast: CharacterMeta[]) {
    const entries = await Promise.all(
      cast.map(async (c) => {
        const [image, avatar] = await Promise.all([
          invoke<string | null>("read_character_image", { worldId, characterId: c.id }).catch(() => null),
          invoke<string | null>("read_character_avatar", { worldId, characterId: c.id }).catch(() => null),
        ]);
        return [c.id, image, avatar] as const;
      }),
    );
    setCharacterImages(
      Object.fromEntries(
        entries.filter(([, image]) => image !== null).map(([id, image]) => [id, `data:image/png;base64,${image}`]),
      ),
    );
    setCharacterAvatars(
      Object.fromEntries(
        entries.filter(([, , avatar]) => avatar !== null).map(([id, , avatar]) => [id, `data:image/png;base64,${avatar}`]),
      ),
    );
  }

  async function loadGmImage(worldId: string) {
    const image = await invoke<string | null>("read_gm_image", { worldId }).catch(() => null);
    setGmImage(image ? `data:image/png;base64,${image}` : null);
  }

  async function loadPlayerCard(worldId: string, playerCardId: string | null) {
    if (!playerCardId) {
      setPlayerCard(null);
      setPlayerImage(null);
      setPlayerAvatar(null);
      return;
    }
    try {
      const [card, image, avatar] = await Promise.all([
        invoke<CharacterCard>("read_character", { worldId, characterId: playerCardId }),
        invoke<string | null>("read_character_image", { worldId, characterId: playerCardId }).catch(() => null),
        invoke<string | null>("read_character_avatar", { worldId, characterId: playerCardId }).catch(() => null),
      ]);
      setPlayerCard(card);
      setPlayerImage(image ? `data:image/png;base64,${image}` : null);
      setPlayerAvatar(avatar ? `data:image/png;base64,${avatar}` : null);
    } catch {
      setPlayerCard(null);
      setPlayerImage(null);
      setPlayerAvatar(null);
    }
  }

  // 語系跟著 config 走；render 前同步進 i18n 模組，之後子樹的 t() 都拿到正確語言
  const language = normalizeLang(config?.preferences["language"]);
  setLang(language);

  // 外觀類偏好（語言、文字大小）：改了立即生效並寫回 config，不設儲存鈕
  async function changePreference(key: string, value: unknown) {
    const current = chatConfigRef.current;
    if (!current) return;
    const updated = { ...current, preferences: { ...current.preferences, [key]: value } };
    chatConfigRef.current = updated;
    setConfig(updated);
    try {
      await invoke("write_config", { config: updated });
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 串流期間 config 可能已被設定頁改寫，走 ref 取最新值，避免舊閉包蓋掉剛存的設定
  const chatConfigRef = useRef(config);
  chatConfigRef.current = config;
  async function markCliConnectedFromChat() {
    const current = chatConfigRef.current;
    if (!current) return;
    const transport = current.preferences["transport"];
    if (!CLI_IDS.includes(transport as (typeof CLI_IDS)[number]) || current.preferences[cliConnectedKey(String(transport))] === true) {
      return;
    }
    const updated = {
      ...current,
      preferences: { ...current.preferences, [cliConnectedKey(String(transport))]: true },
    };
    try {
      await invoke("write_config", { config: updated });
      setConfig(updated);
    } catch (reason) {
      setError(String(reason));
    }
  }

  const textSize = String(config?.preferences["text_size"] ?? TEXT_SIZE_DEFAULT);
  useEffect(() => {
    document.documentElement.style.fontSize =
      TEXT_SIZE_PX[textSize] ?? TEXT_SIZE_PX[TEXT_SIZE_DEFAULT];
  }, [textSize]);

  useEffect(() => {
    void invoke<boolean>("sponsor_status")
      .then(setSponsorUnlocked)
      .catch(() => {});
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = resolveTheme(config, sponsorUnlocked);
  }, [config, sponsorUnlocked]);

  // 中日韓共用同一批 Unicode 碼位但字形不同，斷行規則也各異，靠 lang 屬性讓 webview 挑對字形
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  // 開 App 直接回上次那桌；一桌都沒有就默默開一桌，零精靈（NewPlan §9.3）
  useEffect(() => {
    // 模型清單背景預熱，不擋開桌：玩家走到設定頁時清單早就備好了
    void prefetchModelCatalogs();
    (async () => {
      try {
        const [worldList, loaded] = await Promise.all([
          invoke<WorldMeta[]>("list_worlds"),
          invoke<AppConfig>("read_config"),
        ]);
        setConfig(loaded);
        if (worldList.length === 0) {
          // 首開：語言跟系統語系走並存起來，範例桌直接用該語系生，不擋選語言畫面（設定頁可改）
          let start = loaded;
          if (start.preferences["language"] === undefined) {
            start = { ...start, preferences: { ...start.preferences, language: detectLang() } };
            await invoke("write_config", { config: start });
            setConfig(start);
          }
          const id = await invoke<string>("create_sample_world", {
            lang: normalizeLang(start.preferences["language"]),
          });
          setWorlds(await invoke<WorldMeta[]>("list_worlds"));
          await enterTable(id, start);
          return;
        }
        setWorlds(worldList);
        const last = String(loaded.preferences["last_world"] ?? "");
        const startId = worldList.some((w) => w.id === last) ? last : worldList[0].id;
        await enterTable(startId, loaded);
      } catch (reason) {
        setError(String(reason));
      }
    })();
  }, []);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [events, generating, streamText]);

  // 從世界設定／卡片編輯／單幕閱讀切回對話時，訊息區是重新掛載的，直接跳到底（不跑動畫）
  useEffect(() => {
    if (mainView === null) bottomRef.current?.scrollIntoView();
  }, [mainView]);

  // 切桌、匯入卡、改完世界書都要重問一次這桌有沒有狀態列
  useEffect(() => {
    if (!table) return;
    let stale = false;
    invoke<boolean>("world_has_state_bar", { worldId: table })
      .then((has) => {
        if (!stale) setHasStateBar(has);
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [table, mainView, characters]);

  // 切桌重問這桌各卡的介面腳本；先清空避免上一桌的介面殼閃現，讀失敗就當這桌沒有
  useEffect(() => {
    setCardInterfaces([]);
    if (!table) return;
    let stale = false;
    invoke<CardInterface[]>("card_interfaces", { worldId: table })
      .then((list) => {
        if (!stale) setCardInterfaces(list);
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [table]);

  // 切桌重問這桌的 AI 重構介面殼；沒重構過或那次沒產殼就是 null，cardInterfaceShell 退回既有找殼路徑
  useEffect(() => {
    setRefactorShell(null);
    if (!table) return;
    let stale = false;
    invoke<string | null>("refactor_interface_shell", { worldId: table })
      .then((shell) => {
        if (!stale) setRefactorShell(shell);
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [table]);

  // 目前要顯示的卡片介面殼：AI 重構產過介面產物就優先用它，沒有才退回既有「近 10 則掃 event.raw」路徑。
  // 重構產物兩種：整頁 HTML（舊制殼，狀態樹填值直接顯示）；XML 骨架（照搬卡的每回合輸出格式，
  // 填值後要過卡自己的顯示腳本 regex＋模板才是畫面，`{{本回合.正文}}` 吃最新一則 GM 訊息正文）。
  const cardInterfaceShell = useMemo(() => {
    if (refactorShell !== null) {
      if (/<!DOCTYPE|<html/i.test(refactorShell)) return fillShellPlaceholders(refactorShell, tableTree);
      const latestGm = [...events].reverse().find((event) => event.kind !== "player");
      if (latestGm !== undefined) {
        // 先照直玩語意讓卡腳本試原文：開場（選角）這類訊息卡自己就畫得出來，
        // 硬塞進骨架反而讓兩支腳本互咬（選角殼插進主介面模板中間，抽殼變碎片）
        const direct = findShell(cardInterfaces, [latestGm.raw ?? latestGm.text]);
        if (direct !== null) return direct;
        const filled = fillSkeletonPlaceholders(refactorShell, {
          ...tableTree,
          本回合: { 正文: latestGm.text },
        });
        const fromSkeleton = findShell(cardInterfaces, [filled]);
        if (fromSkeleton !== null) return fromSkeleton;
      }
      // 剛開桌還沒有 GM 回合，或骨架沒過卡的顯示腳本：退回既有路徑（開場白選角殼等）
    }
    const recent = events
      .slice(-10)
      .filter((event) => event.kind !== "player")
      .reverse()
      .map((event) => event.raw ?? event.text);
    // 空桌退回卡片自己的開場白：這類卡的開場就是一整頁選角畫面，玩家得先在那裡選了才有第一句話
    const openings = events.length === 0 ? cardInterfaces.map((card) => card.opening) : [];
    return findShell(cardInterfaces, [...recent, ...openings]);
  }, [refactorShell, tableTree, events, cardInterfaces]);

  const cardShellReady = cardInterfaceShell !== null;

  // 殼的沙盒包裝與內容指紋：指紋當 iframe key，殼一換整支 iframe 重掛——初始掛載必然載入
  // srcdoc，不依賴 WebKit 對 srcDoc 屬性更新／load 事件的行為（雙緩衝翻面機制在 WKWebView
  // 上塞殼與翻面都不可靠，三次卡片介面空白事故後整台拆除，換單 iframe 直繪）。
  // 存下的卡片設定在這裡讀進殼。刻意不進依賴：卡片一存設定就重算 doc 的話，srcdoc 跟著換，
  // 玩家拉個字級就整支 iframe 重繪閃白——殼本來就要重掛的時候（殼變了）才順手帶上最新的一份。
  const cardShellDoc = useMemo(
    () => (cardInterfaceShell === null ? null : buildShellDocument(cardInterfaceShell, readCardStorage(table))),
    [cardInterfaceShell, table],
  );
  const cardShellKey = useMemo(() => shellFingerprint(cardInterfaceShell), [cardInterfaceShell]);

  // 每次 render 換上最新的送出函式：訊息監聽只掛一次，不能讓它抓著開面板當下的舊狀態
  const submitTextRef = useRef((_text: string) => Promise.resolve());
  submitTextRef.current = submitText;

  // 卡片介面殼裡的按鈕經 postMessage 把文字丟回來，直接送出、畫面留在介面裡等回覆——
  // 跟 ST 一樣不必進出對話，也不擋卡片自己觸發回合（那在 ST 上是正常用法，會壞的卡在 ST 也會壞）
  useEffect(() => {
    if (!cardUiOpen) return;
    const onMessage = (event: MessageEvent) => {
      const data = event.data;
      if (typeof data !== "object" || data === null || data.source !== "table-tavern-card") return;
      // 卡片存設定：只落到宿主存檔，不碰 state——這裡一改 state 就會連動 srcdoc 重繪
      if (data.kind === "storage") {
        writeCardStorage(table, data.entries);
        return;
      }
      if (data.kind !== "input") return;
      void submitTextRef.current(String(data.text ?? ""));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [cardUiOpen, table]);

  // Esc 關閉卡片介面覆蓋層；只在開著時掛，避免和其他 Esc 行為（如取消改名）互相搶
  useEffect(() => {
    if (!cardUiOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setCardUiOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cardUiOpen]);

  async function enterTable(id: string, loaded: AppConfig) {
    const state = await invoke<WorldState>("read_state", { worldId: id });
    const transcript = await invoke<TranscriptEvent[]>("read_transcript", {
      worldId: id,
      scene: state.current_scene,
    });
    const cast = await invoke<CharacterMeta[]>("list_characters", { worldId: id });
    setTable(id);
    setScene(state.current_scene);
    setSceneTitles(state.scene_titles ?? {});
    setSceneLabels(state.scene_labels ?? {});
    setTableState(state.state?.table ?? {});
    setTableTree(state.state?.tree ?? {});
    setTableJumps(state.state?.jumps ?? {});
    setBranchBindings(await loadBranchBindings(id));
    setEvents(transcript);
    setCharacters(cast);
    // 本幕已出場集合：auto_hidden 卡是否落在主區靠這份初始化，讀不到就當空集合（全部從隱藏區起算）
    const appearances = await invoke<{ character_ids: string[]; person_titles: string[] }>(
      "scene_appearances",
      { worldId: id },
    ).catch(() => ({ character_ids: [], person_titles: [] }));
    const appearanceIds = new Set(appearances.character_ids);
    setSceneAppearances(appearanceIds);
    await loadCharacterImages(id, cast);
    await loadGmImage(id);
    await loadPlayerCard(id, state.player_card_id);
    setImportReceipts(
      await invoke<ImportReceiptSummary[]>("list_import_receipts", { worldId: id }).catch(() => []),
    );
    setChattedSinceImport(localStorage.getItem(chattedKey(id)) === "true");
    // 一個角色都沒有的桌（純世界書開局）對象預設 GM：不然送出去沒人接、輸入框也是鎖的；
    // 隱藏區的卡（含本幕還沒出場的 auto_hidden）不當預設對象，跟側欄主區顯示一致
    setSpeaker(cast.find((character) => !isCharacterHidden(character, appearanceIds))?.id ?? GM_TARGET);
    setEditingName(null);
    setEditingStateField(null);
    // 切桌就離開單幕閱讀／編輯畫面與前幕浮層，避免殘留上一桌的狀態
    setMainView(null);
    setActsOpen(false);
    setCardUiOpen(false);
    if (loaded.preferences["last_world"] !== id) {
      const next = { ...loaded, preferences: { ...loaded.preferences, last_world: id } };
      await invoke("write_config", { config: next });
      setConfig(next);
    }
  }

  async function switchTable(id: string) {
    if (!config || id === table || generating !== null) return;
    setError("");
    try {
      const previous = table;
      await enterTable(id, config);
      if (previous) await reclaimIfUntouched(previous);
      setWorlds(await invoke<WorldMeta[]>("list_worlds"));
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 還掛著自動名（「新桌」「新桌 2」…）＝使用者沒投入過命名
  function hasAutoName(name: string | undefined) {
    const base = t("newTableName");
    return name === base || (name?.startsWith(`${base} `) ?? false);
  }

  // 空桌回收（NewPlan §9.3）：零訊息、零角色、無設定的桌離開時自動收掉。
  // 但名字改過就代表使用者投入過，即使還沒放內容也不回收——只回收還掛著自動名的桌。
  async function reclaimIfUntouched(id: string) {
    if (!hasAutoName(worlds.find((w) => w.id === id)?.name)) return;
    await invoke("reclaim_world_if_empty", { worldId: id });
  }

  // 一桌一卡：匯入成功後，還掛自動名的桌直接改成卡名；自訂過名字的桌不動
  async function adoptImportName(name: string | null | undefined) {
    const trimmed = name?.trim();
    if (!trimmed) return;
    const oldName = worlds.find((w) => w.id === table)?.name;
    if (!hasAutoName(oldName)) return;
    try {
      await invoke("rename_world", { worldId: table, newName: trimmed });
      setWorlds((previous) => previous.map((w) => (w.id === table ? { ...w, name: trimmed } : w)));
      // 把舊桌名補進這次匯入的收據：復原時桌名才退得回去；記帳失敗不影響改名已經成功
      if (oldName !== undefined) {
        await invoke("record_import_rename", { worldId: table, oldName }).catch(() => {});
      }
    } catch {
      // 改名失敗不影響匯入，桌名維持原樣
    }
  }

  async function newTable() {
    if (!config || generating !== null) return;
    setError("");
    try {
      const existingNames = worlds.map((w) => w.name);
      const base = t("newTableName");
      let name = base;
      for (let n = 2; existingNames.includes(name); n += 1) name = `${base} ${n}`;
      const id = await invoke<string>("create_world", { name });
      const previous = table;
      setWorlds(await invoke<WorldMeta[]>("list_worlds"));
      await enterTable(id, config);
      if (previous) {
        await reclaimIfUntouched(previous);
        setWorlds(await invoke<WorldMeta[]>("list_worlds"));
      }
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function generateTableOutline() {
    const input = genInput.trim();
    if (!input && genGenres.length === 0) return;
    setGenBusy("outline");
    setGenOutline(null);
    setGenOutlineRaw(null);
    setGenResultRaw(null);
    setGenResultMessage("outline");
    setGenError("");
    try {
      const result = await invoke<GenerateOutlineResult>("generate_table_outline", {
        input,
        genres: genGenres.map((key) => t(key as typeof GENRE_KEYS[number])),
      });
      setGenOutlineRaw(result.raw);
      if (result.parsed) {
        setGenOutline(result.parsed);
      } else {
        setGenResultRaw(result.raw);
      }
    } catch (reason) {
      setGenError(String(reason));
    } finally {
      setGenBusy(null);
    }
  }

  async function generateTableCharacter() {
    if (!genOutline) return;
    setGenBusy("character");
    setGenResultRaw(null);
    setGenResultMessage("character");
    setGenError("");
    try {
      const result = await invoke<GenerateCharacterResult>("generate_table_character", {
        input: genInput.trim(),
        genres: genGenres.map((key) => t(key as typeof GENRE_KEYS[number])),
        outlineRaw: serializeGeneratedOutline(genOutline),
        hint: genCharacterHint,
      });
      if (result.parsed) {
        const character = result.parsed;
        setGenOutline((current) => current && {
          ...current,
          characters: [...current.characters, character],
        });
        setGenCharacterHint("");
      } else {
        setGenResultRaw(result.raw);
      }
    } catch (reason) {
      setGenError(String(reason));
    } finally {
      setGenBusy(null);
    }
  }

  async function createGeneratedTable() {
    if (!genOutline || !genOutline.title.trim() || !genOutline.world.trim()) return;
    const input = genInput.trim();
    setGenBusy("expand");
    setGenError("");
    setGenResultRaw(null);
    setGenResultMessage("outline");
    try {
      const result = await invoke<GenerateExpandResult>("generate_table_expand", {
        input,
        genres: genGenres.map((key) => t(key as typeof GENRE_KEYS[number])),
        outlineRaw: serializeGeneratedOutline(genOutline),
      });
      if (!result.worldId) {
        setGenResultRaw(result.raw);
        return;
      }
      setWorlds(await invoke<WorldMeta[]>("list_worlds"));
      await enterTable(result.worldId, config!);
      setGenTableOpen(false);
      setGenOutline(null);
      setGenOutlineRaw(null);
      setGenResultRaw(null);
    } catch (reason) {
      setGenError(String(reason));
    } finally {
      setGenBusy(null);
    }
  }

  // 刪桌：整桌的角色、紀錄、世界設定一起沒，故確認框把後果講白。
  // 刪掉最後一桌就補一張範例桌——App 不留「沒有桌」的空狀態（NewPlan §9.3 零精靈）
  async function deleteTable(id: string) {
    if (!config || generating !== null) return;
    const displayName = worlds.find((w) => w.id === id)?.name ?? id;
    const accepted = await confirm(t("deleteTableConfirm", { name: displayName }), {
      title: t("deleteTableTitle"),
      kind: "warning",
    });
    if (!accepted) return;
    setError("");
    try {
      await invoke("delete_world", { worldId: id });
      let list = await invoke<WorldMeta[]>("list_worlds");
      if (list.length === 0) {
        await invoke<string>("create_sample_world", {
          lang: normalizeLang(config.preferences["language"]),
        });
        list = await invoke<WorldMeta[]>("list_worlds");
      }
      setWorlds(list);
      if (id === table) await enterTable(list[0].id, config);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function renameTable(raw: string) {
    const name = raw.trim();
    setEditingName(null);
    const current = worlds.find((w) => w.id === table);
    if (!current || !name || name === current.name) return;
    setError("");
    try {
      await invoke("rename_world", { worldId: table, newName: name });
      setWorlds((previous) => previous.map((w) => (w.id === table ? { ...w, name } : w)));
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 改桌名的輸入框（主欄標題與側欄共用）：包成表單讓 Enter 走瀏覽器的表單送出，
  // 中文輸入法組字中的 Enter 會被輸入法吃掉（對話輸入框同款做法），不會誤判成確認改名
  function renameForm(className: string) {
    const value = editingName?.value ?? "";
    return (
      <form
        className="table-title-form"
        onSubmit={(e) => {
          e.preventDefault();
          renameTable(value);
        }}
      >
        <input
          className={className}
          autoFocus
          value={value}
          aria-label={t("tableNameAria")}
          onChange={(e) => {
            const next = e.currentTarget.value;
            setEditingName((previous) => (previous ? { ...previous, value: next } : previous));
          }}
          onBlur={() => renameTable(value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") setEditingName(null);
          }}
        />
      </form>
    );
  }

  // 儲存前關掉輸入框，讓失敗時不會卡在一個可能已過期的欄位值上。
  async function saveStateField(path: string[], tree: boolean, value: string) {
    if (stateFieldSaveBusy.current || stateFieldEditCancelled.current) return;
    setEditingStateField(null);
    if (value === (tree ? treeValueAt(tableTree, path) : (tableState[path[0]] ?? ""))) return;
    stateFieldSaveBusy.current = true;
    setError("");
    try {
      if (tree) await invoke("set_state_path", { worldId: table, path, value });
      else await invoke("set_table_state", { worldId: table, fields: { [path[0]]: value } });
      await refreshTableState();
    } catch (reason) {
      setError(String(reason));
    } finally {
      stateFieldSaveBusy.current = false;
    }
  }

  // 表單交給瀏覽器處理 Enter，中文輸入法選字時不會提前送出。
  function stateFieldForm(path: string[], tree: boolean, label: string) {
    const value = editingStateField?.value ?? "";
    return (
      <form
        className="state-bar-field-form"
        onSubmit={(event) => {
          event.preventDefault();
          void saveStateField(path, tree, value);
        }}
      >
        <input
          className="state-bar-input"
          autoFocus
          value={value}
          aria-label={label}
          onChange={(event) => {
            const next = event.currentTarget.value;
            setEditingStateField((previous) => (previous ? { ...previous, value: next } : previous));
          }}
          onBlur={() => void saveStateField(path, tree, value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              stateFieldEditCancelled.current = true;
              setEditingStateField(null);
            }
          }}
        />
      </form>
    );
  }

  // 一列點著就能改的欄位：平欄與樹葉子共用，差別只在存回哪裡
  function stateLeafRow(path: string[], tree: boolean, label: string) {
    const editing =
      editingStateField?.tree === tree &&
      editingStateField.path.length === path.length &&
      editingStateField.path.every((segment, index) => segment === path[index]);
    const value = tree ? treeValueAt(tableTree, path) : (tableState[path[0]] ?? "");
    const jumpMark = tableJumps[path.join(".")];
    return (
      <div className="state-bar-field" key={path.join("\0")}>
        <span className="state-bar-label">{label}</span>
        {editing ? (
          stateFieldForm(path, tree, label)
        ) : (
          <div className="state-bar-value-row">
            <button
              className="state-bar-value"
              type="button"
              title={t("stateEditHint")}
              onClick={() => {
                stateFieldEditCancelled.current = false;
                setEditingStateField({ path, tree, value });
              }}
            >
              {value ? displayUserMacro(value, playerCard?.name || t("playerLabel")) : t("stateEmptyValue")}
            </button>
            {jumpMark && (
              <button
                className="state-bar-jump"
                type="button"
                title={t("stateJumpHint")}
                onClick={() => void markStateCounter(path)}
              >
                {"⚠ " + jumpMark}
              </button>
            )}
          </div>
        )}
      </div>
    );
  }

  // 玩家卡目前指認到的分支路徑，含所有祖先（自己與上面每一層都要預設展開），沒指認就是空集合
  const openBranchPaths = useMemo(() => {
    const bound = playerCard && branchBindings.find((b) => b.characterId === playerCard.id);
    const set = new Set<string>();
    if (bound) {
      for (let depth = 1; depth <= bound.path.length; depth += 1) {
        set.add(bound.path.slice(0, depth).join("/"));
      }
    }
    return set;
  }, [playerCard, branchBindings]);

  // 樹狀折疊：分支一層層收起來，預設展開第一層與玩家自己那支；summary 上附分支指認下拉
  function stateTreeNodes(nodes: Record<string, StateNode>, path: string[], depth: number) {
    return Object.entries(nodes).map(([key, node]) => {
      const childPath = [...path, key];
      if (typeof node === "string") return stateLeafRow(childPath, true, key);
      const bound = branchBindings.find(
        (binding) =>
          binding.path.length === childPath.length &&
          binding.path.every((segment, index) => segment === childPath[index]),
      );
      // 清單出身的分支（鍵全是數字索引，如「氣味標記者：0→利格魯德」的名冊）不是誰的狀態包，
      // 掛指認下拉會被讀成「挑誰出場」；只有名字鍵的物件分支才提供指認。
      const isList = Object.keys(node).every((childKey) => /^\d+$/.test(childKey));
      return (
        <details
          className="state-tree-branch"
          key={key}
          open={depth === 0 || openBranchPaths.has(childPath.join("/"))}
        >
          <summary>
            {key}
            {characters.length > 0 && !isList && (
              <select
                className="state-tree-bind"
                aria-label={t("stateBranchBindAria")}
                title={t("stateBranchBindHint")}
                value={bound?.characterId ?? ""}
                onClick={(event) => event.stopPropagation()}
                onPointerDown={(event) => event.stopPropagation()}
                onChange={(event) => {
                  const nextId = event.currentTarget.value;
                  if (nextId) void bindBranch(nextId, childPath);
                  else if (bound) void bindBranch(bound.characterId, null);
                }}
              >
                <option value="">{t("stateBranchUnbound")}</option>
                {activeCharacters.map((character) => (
                  <option key={character.id} value={character.id}>
                    {character.name}
                  </option>
                ))}
              </select>
            )}
          </summary>
          <div className="state-tree-children">{stateTreeNodes(node, childPath, depth + 1)}</div>
        </details>
      );
    });
  }

  async function refreshTableState() {
    const state = await invoke<WorldState>("read_state", { worldId: table });
    setTableState(state.state?.table ?? {});
    setTableTree(state.state?.tree ?? {});
    setTableJumps(state.state?.jumps ?? {});
    setBranchBindings(await loadBranchBindings(table));
  }

  // 玩家點跳動記號：把該欄永久標成計數器，之後全量桌跳動比對不再對它示警
  async function markStateCounter(path: string[]) {
    setError("");
    try {
      await invoke("mark_state_counter", { worldId: table, path });
      await refreshTableState();
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 綁定清單載入失敗就當空陣列——面板本來就能在沒有綁定資料時正常運作，不因此整個掛掉
  async function loadBranchBindings(worldId: string): Promise<BranchBinding[]> {
    try {
      return await invoke<BranchBinding[]>("branch_bindings", { worldId });
    } catch {
      return [];
    }
  }

  // 指認／解除分支給角色；成功後重載綁定清單，失敗照面板既有規矩交給 setError
  async function bindBranch(characterId: string, path: string[] | null) {
    try {
      await invoke("set_branch_binding", { worldId: table, characterId, path });
      setBranchBindings(await loadBranchBindings(table));
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 換場：把目前場景公開紀錄壓成一則前情提要，寫進新場景開頭，current_scene +1
  async function advanceScene() {
    setError("");
    setGenerating({ id: "", kind: "narration" });
    setStreamText("");
    try {
      await invoke<number>("advance_scene", { worldId: table });
      await enterTable(table, config!);
      noteTurnDone();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }

  // 從前幕分岔續玩：把那一幕的紀錄複製成新的一幕，原本的歷史原封不動。
  // 整幕複製會讓下一次生成要送的內容變多，所以先跳確認框讓玩家自己決定
  async function forkScene(from: number) {
    const accepted = await confirm(t("sceneForkConfirm"), {
      title: t("sceneForkTitle"),
      kind: "warning",
    });
    if (!accepted) return;
    setError("");
    try {
      await invoke<number>("fork_scene", { worldId: table, scene: from });
      setMainView(null);
      await enterTable(table, config!);
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 太早按到換幕的補救：這一幕還只有那則前情提要時，刪掉它退回上一幕接著玩。
  // 前幕紀錄從來沒被動過（換幕只是開新檔），所以退回不會掉任何內容
  async function revertScene() {
    if (!canUndoScene) return;
    setError("");
    try {
      await invoke<number>("revert_scene", { worldId: table });
      await enterTable(table, config!);
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 前情提要不滿意就重寫一份。拿前幕原始紀錄重跑一次摘要，蓋掉這幕唯一那則
  async function regenerateSummary() {
    if (!canUndoScene) return;
    setError("");
    setGenerating({ id: "", kind: "narration" });
    setStreamText("");
    try {
      await invoke("regenerate_scene_summary", { worldId: table });
      await enterTable(table, config!);
      noteTurnDone();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }

  // 存哪裡由使用者決定：跳原生「另存新檔」對話框，取消就什麼都不做
  async function exportTranscript() {
    setError("");
    try {
      const now = new Date();
      const pad = (n: number) => String(n).padStart(2, "0");
      const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}${pad(now.getMinutes())}`;
      const path = await saveDialog({
        defaultPath: `${t("exportFileName", { table: tableName, stamp })}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      await invoke("export_transcript", { worldId: table, path });
      await revealItemInDir(path);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function refreshCardInterfaces(worldId: string) {
    const list = await invoke<CardInterface[]>("card_interfaces", { worldId }).catch(
      () => [] as CardInterface[],
    );
    setCardInterfaces(list);
    return list;
  }

  async function refreshRefactorShell(worldId: string) {
    const shell = await invoke<string | null>("refactor_interface_shell", { worldId }).catch(() => null);
    setRefactorShell(shell);
    return shell;
  }

  // 匯入完畫得出來就直接打開一次：這類卡的開場本來就是一整頁畫面，
  // 玩家不主動點按鈕不會知道有這東西（聊天裡只看得到孤零零一句「请选择你的身份」）
  function openCardInterface(list: CardInterface[]) {
    if (findShell(list, list.map((card) => card.opening)) !== null) setCardUiOpen(true);
  }

  async function refreshImportReceipts(worldId: string) {
    setImportReceipts(
      await invoke<ImportReceiptSummary[]>("list_import_receipts", { worldId }).catch(() => []),
    );
    // 剛匯入（或剛復原一筆）＝又回到「還沒開演」的狀態，按鈕重新給
    localStorage.removeItem(chattedKey(worldId));
    setChattedSinceImport(false);
  }

  // 向 AI 發出對話請求：從這一刻起收掉「復原上次匯入」，免得演到一半誤按整張卡沒了
  function noteChatRequest() {
    if (!table) return;
    localStorage.setItem(chattedKey(table), "true");
    setChattedSinceImport(true);
  }

  // 側欄「復原上次匯入」：逆向收據清單最後一筆，逐筆倒退
  async function undoLastImport() {
    if (importReceipts.length === 0) return;
    setError("");
    const last = importReceipts[importReceipts.length - 1];
    try {
      const accepted = await confirm(t("undoLastImportConfirm", { label: last.label }), {
        title: t("undoLastImport"),
        kind: "warning",
      });
      if (!accepted) return;
      const report = await invoke<UndoReport>("undo_last_import", { worldId: table });
      const cast = await refreshCharacters();
      // 發言對象指向的角色被這次復原刪掉了（不管是不是巧合）就改回 GM，不然輸入框對著空氣
      if (speaker && speaker !== GM_TARGET && !cast.some((character) => character.id === speaker)) {
        setSpeaker(GM_TARGET);
      }
      await refreshCardInterfaces(table);
      // 復原的若是重構套用，磁碟上的介面殼檔已被刪，前端快取跟著重問一次
      await refreshRefactorShell(table);
      // 復原的若是 PNG 世界書匯入，GM 卡的圖也被刪了，重讀一次回到書本圖
      await loadGmImage(table);
      // 貼出的開場白被一起收掉：檯面與狀態快照都變了，重讀這一幕
      if (report.removed_opening) {
        setEvents(await invoke<TranscriptEvent[]>("read_transcript", { worldId: table, scene }));
        await refreshTableState();
      }
      setWorlds(await invoke<WorldMeta[]>("list_worlds"));
      // 世界設定畫面（世界書／機制帳本）若開著，資料在它自己的元件狀態裡，用 key 強制整個重掛載重載
      setWorldEditorRefreshKey((key) => key + 1);
      await refreshImportReceipts(table);
      await showMessage(
        t("undoLastImportDone") +
          (report.removed_characters.length > 0
            ? t("undoLastImportRemovedCharacters", { names: report.removed_characters.join("、") })
            : "") +
          (report.kept_entries > 0 ? t("undoLastImportKept", { n: report.kept_entries }) : ""),
        { title: t("undoLastImport") },
      );
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 匯入 SillyTavern 角色卡（V2 PNG 或 JSON）：讀 bytes 交後端探測，依探測結果分流——
  // 純世界書、純角色卡都零詢問直接判定身分（還要再過第二張卡路由）；
  // 角色與世界書兩種身分都有料才彈三鍵對話框問玩家要哪個，答完一樣過路由。
  async function importCharacter(file: File) {
    setError("");
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const data = Array.from(bytes);
      let probe: ImportProbe = {
        lorebook_heavy: false,
        parsed: false,
        book_entries: 0,
        book_shaped: false,
        alternate_greetings: 0,
      };
      try {
        probe = await invoke<ImportProbe>("probe_import", { data });
      } catch {
        // 探測失敗不擋匯入：舊版後端或格式未知時照原流程走。
      }
      if (probe.parsed && (!probe.name || probe.book_shaped)) {
        // 純世界書檔（含自帶書名的 V2 獨立書）：沒有角色可建，「匯入成角色卡」是假選項，不問
        await routeImport("worldbook", data, probe.name ?? file.name.replace(/\.[^.]+$/, ""));
        return;
      }
      if (probe.parsed && probe.name) {
        // 其餘一律問身分：判準只決定主按鈕，判錯玩家仍有另一條路（見 booksFirst）
        setImportChoice({ data, name: probe.name, booksFirst: looksLikeWorldbook(probe) });
        return;
      }
      // 解析失敗：照舊走角色路徑，讓後端報原本的格式錯誤，不算第二張卡場景，不過路由
      await importAsCharacter(table, data);
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 三鍵對話框的作答：取消什麼都不做，另外兩個選項答出身分後都要過第二張卡路由
  async function answerImportChoice(choice: "character" | "worldbook" | "cancel") {
    const pending = importChoice;
    setImportChoice(null);
    if (!pending || choice === "cancel") return;
    setError("");
    try {
      await routeImport(choice, pending.data, pending.name);
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 第二張卡路由：身分已定，看這桌現況決定要不要跳提醒框。
  // direct 零打擾直接匯；ask／merge_worldbook 開框，框裡選完才真的匯（見 answerImportRoute）。
  async function routeImport(identity: "character" | "worldbook", data: number[], label: string) {
    // 收據為空才問條目：那可能是收據功能之前的舊桌、手建的桌或範例桌
    const needsFallback = identity === "worldbook" && importReceipts.length === 0;
    const route = decideImportRoute(
      identity,
      importReceipts.map((receipt) => receipt.kind),
      needsFallback && (await tableHasWorldbookEntries()),
    );
    if (route === "direct") {
      if (identity === "worldbook") await importAsWorldbook(table, data, label);
      else await importAsCharacter(table, data);
      return;
    }
    setImportRoute({ data, identity, label, route });
  }

  // 收據為空時的保險（見 decideImportRoute）。現讀而不吃 state：世界書條目歸 WorldEditor 管，
  // App 這邊沒有同步的一份。讀不到就當有——寧可多問一句，也不要把書無聲合進有內容的桌。
  async function tableHasWorldbookEntries() {
    try {
      return (await invoke<WorldbookEntry[]>("read_worldbook", { worldId: table })).length > 0;
    } catch {
      return true;
    }
  }

  // 路由框作答：取消什麼都不做；匯進這桌走現行匯入函式（第二本世界書走同一條，後端會接在既有條目後面並去重）；
  // 開新桌並匯入另開一桌後再匯，見 openNewTableAndImport
  async function answerImportRoute(choice: "this_table" | "new_table" | "cancel") {
    const pending = importRoute;
    setImportRoute(null);
    if (!pending || choice === "cancel") return;
    setError("");
    try {
      if (choice === "this_table") {
        if (pending.identity === "worldbook") {
          await importAsWorldbook(table, pending.data, pending.label);
        } else {
          await importAsCharacter(table, pending.data);
        }
      } else {
        await openNewTableAndImport(pending);
      }
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 開新桌並匯入：桌名直接用卡名／書名／檔名（pending.label），create_world 回傳的新 id
  // 全程顯式帶入（不靠 table 這個 closure，切桌當下它還是舊值），原桌完全不動（不回收、不改名）。
  // adoptImportName 不需要跑：新桌從一開始就用 label 命名。沿用 newTable／switchTable 的生成中防呆。
  async function openNewTableAndImport(pending: {
    data: number[];
    identity: "character" | "worldbook";
    label: string;
  }) {
    if (!config || generating !== null) return;
    const id = await invoke<string>("create_world", { name: pending.label });
    setWorlds(await invoke<WorldMeta[]>("list_worlds"));
    await enterTable(id, config);
    if (pending.identity === "worldbook") {
      await importAsWorldbook(id, pending.data, pending.label, false);
    } else {
      await importAsCharacter(id, pending.data, false);
    }
  }

  // worldId 顯式帶入（不吃 table 這個 closure）：開新桌並匯入時 table 當下還是舊桌值。
  // adoptName 預設 true；開新桌路徑傳 false——新桌從建立那刻就已經用卡名命名，不必再改一次。
  async function importAsCharacter(worldId: string, data: number[], adoptName = true) {
    const { meta, book } = await invoke<CharacterImport>("import_character", {
      worldId,
      data,
      color: PALETTE[characters.length % PALETTE.length],
    });
    const cast = await invoke<CharacterMeta[]>("list_characters", { worldId });
    setCharacters(cast);
    await loadCharacterImages(worldId, cast);
    setSpeaker(meta.id);
    if (adoptName) await adoptImportName(meta.name);
    await refreshImportReceipts(worldId);
    // 卡片隨身的世界書條目也要報數，跟世界書路徑講一樣的話
    if (book.imported > 0) await showMessage(worldbookImportedMessage(book), { title: t("importCard") });
    await offerOpeningLine(worldId, data);
    await tellAboutInterface(worldId, meta.id);
  }

  // 世界書匯入共用流程：側欄按鈕分流出的純世界書檔、三鍵對話框選了世界書都走這裡。
  // worldId 顯式帶入、adoptName 預設 true，理由同 importAsCharacter。
  async function importAsWorldbook(worldId: string, data: number[], label: string, adoptName = true) {
    const book = await invoke<WorldbookImport>("import_worldbook", { worldId, data, label });
    // 匯的是 PNG 卡：後端已把整張圖存成 GM 卡的圖，這裡讀回來讓側欄立刻換掉書本圖
    await loadGmImage(worldId);
    await showMessage(worldbookImportedMessage(book), { title: t("importCard") });
    // 世界書更容易不知道怎麼開始：匯完一律把對話目標指到 GM
    setSpeaker(GM_TARGET);
    if (adoptName) await adoptImportName(label);
    await refreshImportReceipts(worldId);
    await offerOpeningLine(worldId, data);
    // 這桌等級的介面殼 character_id 是空字串（角色卡的是那張卡的 id）
    await tellAboutInterface(worldId, "");
  }

  function worldbookImportedMessage(book: WorldbookImport) {
    return (
      t("worldbookImportDone", { n: book.imported }) +
      (book.skipped > 0 ? t("worldbookDuplicatesSkipped", { d: book.skipped }) : "")
    );
  }

  // 兩條匯入路徑共用：畫得出來就告訴玩家在哪開並直接開一次，解不開的講清楚是哪一種
  // （加密卡、介面存在別人網站上的雲端載入器卡）。沒有介面的卡什麼都不說。
  async function tellAboutInterface(worldId: string, characterId: string) {
    const interfaces = await refreshCardInterfaces(worldId);
    const mine = interfaces.find((card) => card.character_id === characterId);
    const notice =
      mine && mine.scripts.length > 0
        ? t("importCardInterface")
        : mine?.unsupported === "scrypt"
          ? t("importCardScrypt")
          : mine?.unsupported === "remote_loader"
            ? t("importCardRemoteLoader")
            : "";
    if (notice) await showMessage(notice, { title: t("importCard") });
    openCardInterface(interfaces);
  }

  // 兩條匯入路徑共用。一律貼成旁白而不是角色發言——開場白不一定是那個角色說的話，
  // 也常是場景或角色本身的描寫。主開場白常是使用說明（真正的劇情藏在備用開場白），
  // 所以列全部讓玩家挑。直接讀匯入檔，不建卡也拿得到
  async function offerOpeningLine(worldId: string, data: number[]) {
    const openings = await invoke<string[]>("card_openings", {
      worldId,
      data,
      lang: language,
    });
    if (openings.length === 0) return;
    setOpeningExpanded(null);
    setOpeningTransState({});
    openingTransAbort.current = false;
    setOpeningChoice(openings);
  }

  // 開場白翻譯：單則呼叫 translate_opening（走 fast 檔，失敗退 GM 檔，見 lib.rs），
  // 兩顆翻譯鈕共用。已經 done 的直接回傳目前內容，不重打；modal 關閉中途 abort 就不再
  // 動 state（視窗都不在了，setState 也只是白費）。
  async function translateOpeningLine(index: number): Promise<string | null> {
    if (openingChoice === null) return null;
    if (openingTransState[index] === "done") return openingChoice[index];
    const text = openingChoice[index];
    setOpeningTransState((previous) => ({ ...previous, [index]: "translating" }));
    try {
      const translated = await invoke<string>("translate_opening", { worldId: table, text, lang: language });
      if (openingTransAbort.current) return null;
      setOpeningChoice((previous) =>
        previous === null ? previous : previous.map((item, itemIndex) => (itemIndex === index ? translated : item)),
      );
      setOpeningTransState((previous) => ({ ...previous, [index]: "done" }));
      return translated;
    } catch (reason) {
      if (openingTransAbort.current) return null;
      setOpeningTransState((previous) => ({ ...previous, [index]: "error" }));
      setError(String(reason));
      return null;
    }
  }

  // 「✨ 全部翻譯」：逐則序列翻譯，不擋操作（沒鎖住 modal 其他按鈕）；modal 一關（abort
  // 旗標翻真）就停止發下一則呼叫，省下不會有人看到的 AI 額度。
  async function translateAllOpenings() {
    if (openingChoice === null || openingTransAllBusy) return;
    setOpeningTransAllBusy(true);
    openingTransAbort.current = false;
    for (let index = 0; index < openingChoice.length; index += 1) {
      if (openingTransAbort.current) break;
      await translateOpeningLine(index);
    }
    setOpeningTransAllBusy(false);
  }

  // 「✨ 翻譯後貼出」：挑中那則已翻好就直接貼出；沒翻就先翻這一則，成功才貼出，
  // 失敗留在原地（原文仍在，原「貼出」鈕照常可按）。
  async function postTranslatedOpening(index: number) {
    if (openingChoice === null) return;
    const translated = await translateOpeningLine(index);
    if (translated !== null) await postOpening(translated);
  }

  async function refreshCharacters() {
    const cast = await invoke<CharacterMeta[]>("list_characters", { worldId: table });
    setCharacters(cast);
    await loadCharacterImages(table, cast);
    return cast;
  }

  // 建卡或改名存檔後：名單與圖片重載；id 全程不變，只有「新卡剛存下」要轉正畫面並選為發言對象
  async function finishCardSaved(id: string) {
    const wasNew = mainView?.kind === "new-character";
    await refreshCharacters();
    if (wasNew) {
      setMainView({ kind: "character", id });
      setSpeaker(id);
    }
  }

  // 玩家卡是桌的屬性：第一次存檔才把 id 掛進 state，之後存檔只重載顯示（比照角色卡留在編輯器）
  async function finishPlayerCardSaved(id: string) {
    if (mainView?.kind === "new-player") {
      const state = await invoke<WorldState>("read_state", { worldId: table });
      await invoke("write_state", { worldId: table, state: { ...state, player_card_id: id } });
      setMainView({ kind: "player", id });
    }
    await loadPlayerCard(table, id);
  }

  // 角色被隱藏或刪除後的善後：名單重載、發言對象改人、關掉編輯面板
  async function finishRemoval(id: string) {
    const cast = await refreshCharacters();
    if (speaker === id) {
      setSpeaker(cast.find((character) => !isCharacterHidden(character, sceneAppearances))?.id ?? "");
    }
    setMainView(null);
  }

  // 編輯角色卡時，側欄點擊＝換編輯對象（發言對象只在聊天畫面有意義），離開前先問未儲存
  async function canLeaveEditor() {
    // 只有掛著未儲存追蹤的三種畫面要問；聊天／幕紀錄沒有暫存狀態，也避免問到已卸載編輯器留下的舊守門
    const guarded = cardView !== null || mainView?.kind === "world";
    if (!guarded) return true;
    const ok = (await leaveGuard.current?.()) ?? true;
    // 放行就清掉，下一張卡載好會重新掛上——免得載入空窗期沿用上一張的未儲存狀態
    if (ok) leaveGuard.current = null;
    return ok;
  }

  async function editCard(id: string) {
    if (mainView?.kind === "character" && mainView.id === id) return;
    if (await canLeaveEditor()) setMainView({ kind: "character", id });
  }

  async function openPlayerCard() {
    if (mainView?.kind === "player" || mainView?.kind === "new-player") return;
    if (!(await canLeaveEditor())) return;
    if (playerCard) {
      setMainView({ kind: "player", id: playerCard.id });
      return;
    }
    try {
      const id = await invoke<string>("new_id");
      setMainView({ kind: "new-player", id });
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 主欄開著任何畫面時側欄＝導覽（點卡＝開它的編輯頁）；只有聊天畫面點卡才是選發言對象
  // 再點一次已選中的卡＝取消對象，讓玩家能描述動作或對全場說話
  async function selectCard(id: string) {
    if (mainView) {
      await editCard(id);
      return;
    }
    setSpeaker((current) => (current === id ? "" : id));
  }

  // GM 卡與角色卡同一套：聊天畫面點擊＝選／取消發言對象，其他畫面＝導覽到世界設定
  async function selectGm() {
    if (mainView) {
      await openWorldEditor();
      return;
    }
    setSpeaker((current) => (current === GM_TARGET ? "" : GM_TARGET));
  }

  async function openWorldEditor() {
    if (mainView?.kind === "world") return;
    if (await canLeaveEditor()) setMainView({ kind: "world" });
  }

  // 建卡先跟後端要一個 id：草稿期生圖就落在正確的圖庫目錄，存檔用同一個 id
  async function openNewCard() {
    if (mainView?.kind === "new-character") return;
    if (!(await canLeaveEditor())) return;
    try {
      const id = await invoke<string>("new_id");
      setMainView({ kind: "new-character", id });
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 側欄拖曳排序：先樂觀套用，寫檔失敗才回捲
  // 用 archivedCharacters（非 characters.filter(archived)）補回其餘卡片：
  // 這幕沒出場的 auto_hidden 卡不在 archived 裡，漏掉會在拖曳當下從 state 消失
  async function reorderCast(ordered: CharacterMeta[]) {
    setError("");
    const previous = characters;
    setCharacters([...ordered, ...archivedCharacters]);
    try {
      await invoke("reorder_characters", {
        worldId: table,
        ids: ordered.map((character) => character.id),
      });
    } catch (reason) {
      setCharacters(previous);
      setError(String(reason));
    }
  }

  async function restoreCharacter(id: string) {
    setError("");
    try {
      await invoke("set_character_archived", { worldId: table, characterId: id, archived: false });
      await refreshCharacters();
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 隱藏區裡 auto_hidden 卡的「拉回」：解除自動隱藏（不是解除封存），下次換幕結算才會重新判定
  async function restoreAutoHidden(id: string) {
    setError("");
    try {
      await invoke("set_character_auto_hidden", { worldId: table, characterId: id, autoHidden: false });
      await refreshCharacters();
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 隱藏區與角色卡編輯畫面共用同一條刪除路徑（確認框＋善後）
  async function deleteCharacter(id: string) {
    setError("");
    try {
      const name = metaOf(id)?.name ?? id;
      const accepted = await confirm(t("deleteCharacterConfirm", { name }), {
        title: t("deleteCharacterTitle"),
        kind: "warning",
      });
      if (!accepted) return;
      await invoke("delete_character", { worldId: table, characterId: id });
      await finishRemoval(id);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function deletePlayerCard(id: string) {
    setError("");
    try {
      const accepted = await confirm(t("deleteCharacterConfirm", { name: playerCard?.name ?? id }), {
        title: t("deleteCharacterTitle"),
        kind: "warning",
      });
      if (!accepted) return;
      await invoke("delete_character", { worldId: table, characterId: id });
      const state = await invoke<WorldState>("read_state", { worldId: table });
      await invoke("write_state", { worldId: table, state: { ...state, player_card_id: null } });
      setPlayerCard(null);
      setPlayerImage(null);
      setPlayerAvatar(null);
      setMainView(null);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function appendEvent(event: TranscriptEvent) {
    await invoke("append_transcript", { worldId: table, scene, event });
    setEvents((previous) => [...previous, event]);
    // 桌上一有新內容，收回的那幾句就不能再放回去了——位置已經被後面的話蓋掉
    setUndone(null);
  }

  async function postOpening(text: string) {
    setError("");
    try {
      const event = await invoke<TranscriptEvent>("post_opening", { worldId: table, scene, ts: nowTs(), text });
      setEvents((previous) => [...previous, event]);
      setUndone(null);
      await refreshTableState();
      setOpeningChoice(null);
    } catch (reason) {
      setError(String(reason));
    }
  }

  // 收回上一句：一次砍一則、可連按往回收，收到這一幕見底就停（不動上一幕）
  async function undoLast() {
    if (generating !== null || events.length === 0 || undoBusy.current) return;
    undoBusy.current = true;
    setError("");
    const last = events[events.length - 1];
    try {
      if (!(await invoke<boolean>("pop_transcript", { worldId: table, scene }))) return;
      setEvents((previous) => previous.slice(0, -1));
      setUndone((previous) =>
        previous && previous.table === table && previous.scene === scene
          ? { ...previous, events: [...previous.events, last] }
          : { table, scene, events: [last] },
      );
      await refreshTableState();
    } catch (reason) {
      setError(String(reason));
    } finally {
      undoBusy.current = false;
    }
  }

  // 復原一次放回一則，可連按把整輪收回逐則倒回去。
  // 這裡不走 appendEvent——放回舊句不該把剩下那幾句一起作廢，只消耗疊頂那一則
  async function restoreUndone() {
    if (!undone || !canRestore || generating !== null || undoBusy.current) return;
    undoBusy.current = true;
    const event = undone.events[undone.events.length - 1];
    setError("");
    try {
      await invoke("append_transcript", { worldId: table, scene, event });
      setEvents((previous) => [...previous, event]);
      setUndone((previous) =>
        previous && previous.events.length > 1
          ? { ...previous, events: previous.events.slice(0, -1) }
          : null,
      );
      await refreshTableState();
    } catch (reason) {
      setError(String(reason));
    } finally {
      undoBusy.current = false;
    }
  }

  // 玩家真的推進了一步：保溫節奏重新開始，離開提示收掉
  function noteTurnDone() {
    lastTurnAt.current = Date.now();
    pingCount.current = 0;
    keepaliveOff.current = false;
    setAwayTooLong(false);
  }

  // 保溫 ping：視窗在前景且距上次推進夠久才發，連三次都沒等到玩家就收手改提示換幕。
  // 視窗不在前景一律不發——人不在還持續扣錢是最糟的情況。
  useEffect(() => {
    if (!table) return;
    const timer = setInterval(async () => {
      if (keepaliveOff.current || generatingRef.current !== null) return;
      if (Date.now() - lastTurnAt.current < KEEPALIVE_AFTER_MS) return;
      if (pingCount.current >= KEEPALIVE_MAX_PINGS) {
        setAwayTooLong(true);
        return;
      }
      if (!document.hasFocus()) return;
      try {
        const lanes = await invoke<number>("keepalive_lanes", { worldId: table });
        // 沒有線可保（非 claude 模式、或這桌還沒開過線）：靜靜停掉，不算進次數也不提示離開
        if (lanes === 0) {
          keepaliveOff.current = true;
          return;
        }
        pingCount.current += 1;
        lastTurnAt.current = Date.now();
      } catch {
        keepaliveOff.current = true;
      }
    }, KEEPALIVE_TICK_MS);
    return () => clearInterval(timer);
  }, [table]);

  // 單次角色接話（不含 busy 防護），供手動點名與 GM 推進共用；失敗往外拋由呼叫端收尾
  async function replyOnce(characterId: string) {
    noteChatRequest();
    setGenerating({ id: characterId, kind: "dialogue" });
    setStreamText("");
    const onDelta = new Channel<string>();
    onDelta.onmessage = (delta) => setStreamText((previous) => previous + delta);
    const full = await invoke<string>("chat_with_character", {
      worldId: table,
      characterId,
      onDelta,
    });
    const name = metaOf(characterId)?.name ?? "";
    await appendEvent({ ts: nowTs(), speaker_id: characterId, speaker_name: name, kind: "dialogue", text: full });
    await markCliConnectedFromChat();
    noteTurnDone();
  }

  // 點名指定角色接話；也是「請 X 發言」按鈕的入口（NewPlan §9、MVP 第 8 項）
  async function requestReply(characterId: string) {
    if (!characterId || generating !== null) return;
    setError("");
    try {
      await replyOnce(characterId);
      setWorlds(await invoke<WorldMeta[]>("list_worlds"));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }

  // 單次 GM 旁白＋點名（不含 busy 防護）：後端一次呼叫完成，旁白落 transcript，
  // 回傳下一位發言者（角色 id／玩家哨兵／null＝GM 沒點名）；失敗往外拋由呼叫端收尾
  async function narrateOnce(): Promise<string | null> {
    noteChatRequest();
    setGenerating({ id: "", kind: "narration" });
    setStreamText("");
    const onDelta = new Channel<string>();
    onDelta.onmessage = (delta) => setStreamText((previous) => previous + delta);
    const { text, raw, next, state_updates, arrived_characters } = await invoke<{
      text: string;
      raw: string | null;
      next: string | null;
      // 後端還沒上線這欄時是 undefined，當空陣列處理，別讓面板炸掉
      state_updates?: { path: string; value: string }[];
      // 這輪劇情帶出場的卡 id：併入本幕出場集合，auto_hidden 卡立刻從隱藏區移回主區
      arrived_characters?: string[];
    }>("gm_narrate", {
      worldId: table,
      onDelta,
    });
    if (arrived_characters && arrived_characters.length > 0) {
      setSceneAppearances((previous) => new Set([...previous, ...arrived_characters]));
    }
    await appendEvent({ ts: nowTs(), speaker_id: "", speaker_name: "GM", kind: "narration", text, ...(raw ? { raw } : {}) });
    // 長文字欄（外貌、貼文…）改用一則系統事件記變動，不再每輪塞回提示詞——
    // 歷史會被兩條傳輸路每輪重播且吃快取，回合尾動態塊每輪重組、不落歷史
    const updates = state_updates ?? [];
    if (updates.length > 0) {
      await appendEvent({
        ts: nowTs(),
        speaker_id: "",
        speaker_name: "GM",
        kind: "system",
        text: [t("stateUpdateHeader"), ...updates.map((u) => `${u.path}：${u.value}`)].join("\n"),
      });
    }
    await refreshTableState();
    await markCliConnectedFromChat();
    noteTurnDone();
    return next;
  }

  // 簡易導演：GM 插入旁白（NewPlan §6.1、MVP 第 9 項）；一併回來的點名這裡不用，讓玩家自己決定下一步
  async function gmNarrate() {
    if (generating !== null) return;
    setError("");
    try {
      await narrateOnce();
      setWorlds(await invoke<WorldMeta[]>("list_worlds"));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }

  // 簡易導演：GM 旁白＋點名→角色接話的接力，至「輪到玩家」、GM 沒點名或每回合上限停下（NewPlan §6.1）
  async function gmAdvance() {
    if (!config || generating !== null || activeCharacters.length === 0) return;
    setError("");
    const max = Math.max(1, Number(config.preferences["max_round_speakers"]) || 3);
    try {
      for (let turn = 0; turn < max; turn += 1) {
        const next = await narrateOnce();
        if (next === null) break;
        // 輪到玩家：一樣留下點名紀錄（球在你手上），但不接話、就此停下
        if (next === PLAYER_SENTINEL) {
          const you = playerCard?.name || t("playerLabel");
          await appendEvent({ ts: nowTs(), speaker_id: "", speaker_name: "GM", kind: "system", text: t("gmCallOn", { name: you }) });
          break;
        }
        const name = metaOf(next)?.name ?? next;
        await appendEvent({ ts: nowTs(), speaker_id: "", speaker_name: "GM", kind: "system", text: t("gmCallOn", { name }) });
        await replyOnce(next);
      }
      setWorlds(await invoke<WorldMeta[]>("list_worlds"));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }

  // 請目前的發言對象接話：GM 以旁白回應（讀得到世界設定與全部角色卡），角色就點名接話
  async function replyFromTarget() {
    if (speaker === GM_TARGET) await gmNarrate();
    else if (speaker) await requestReply(speaker);
  }

  async function send(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await submitText(input);
  }

  async function submitText(raw: string) {
    const text = raw.trim();
    if (generating !== null) return;
    // 卡片只按了 /trigger（沒帶文字）＝直接要對象接話，不留玩家發言
    if (!text) {
      await replyFromTarget();
      return;
    }
    setError("");
    setInput("");
    try {
      await appendEvent({ ts: nowTs(), speaker_id: "", speaker_name: playerCard?.name || t("playerLabel"), kind: "player", text });
    } catch (reason) {
      setError(String(reason));
      return;
    }
    // 沒指定對象＝只把這句留在桌上（描述動作或對全場說），不點名任何人接話
    await replyFromTarget();
  }

  const metaOf = (id: string) => characters.find((c) => c.id === id);

  // 收回過、且還停在同一桌同一幕，才給復原（換桌換幕就當這次收回已成定局）
  const canRestore = undone !== null && undone.table === table && undone.scene === scene;

  // 剛換完幕、這一幕只有那則前情提要＝還沒開始玩，兩條補救路都還來得及。
  // 一有新內容就作廢（同復原疊的道理：位置已被後話蓋掉，退回會連新句子一起丟）。
  // 分岔來的幕排除掉：那一則是複製來的真實對話，重寫會直接把它蓋成摘要
  const canUndoScene =
    scene > 0 &&
    events.length === 1 &&
    generating === null &&
    !sceneLabels[String(scene)]?.forked;

  // 發言對象可能是 GM（沒有角色卡），顯示名與顏色在這裡收斂一次
  const gmTargeted = speaker === GM_TARGET;
  const targetName = gmTargeted ? "GM" : (metaOf(speaker)?.name ?? speaker);
  const requestReplyLabel = t("requestReplyBtn", {
    name: speaker ? targetName : t("characterFallback"),
  });

  // 幕的顯示標籤：有取到幕名就「第 n 幕：幕名」，沒有就沿用「第 n 幕」；n 從 1 起算，內部場號 0 起算。
  // 分岔出來的幕顯示編號跟著源頭走、後面掛版本號（第 1 幕 (2)），沒進 scene_labels 的就是原線
  const sceneDisplayLabel = (n: number) => {
    const title = sceneTitles[String(n)];
    const label = sceneLabels[String(n)];
    const shown = (label?.base ?? n) + 1;
    const v = label?.version ?? 1;
    if (v > 1) {
      return title
        ? t("sceneWithTitleVersioned", { n: shown, v, title })
        : t("sceneLabelVersioned", { n: shown, v });
    }
    return title ? t("sceneWithTitle", { n: shown, title }) : t("sceneLabel", { n: shown });
  };
  const generatingMeta = generating !== null ? metaOf(generating.id) : undefined;

  // 設定頁改語言時：既有範例桌內容還是舊語言，問一次要不要用新語言重生（答過就記住，之後改語言不再問）
  async function changeSettingPreference(key: string, value: unknown) {
    const before = normalizeLang(chatConfigRef.current?.preferences["language"]);
    const asked = chatConfigRef.current?.preferences["sample_regen_asked"] === true;
    await changePreference(key, value);
    if (key === "language" && value !== before && !asked) setRegenAsk(before);
  }

  async function answerRegen(answer: "regen" | "keep" | "cancel") {
    const before = regenAsk;
    setRegenAsk(null);
    if (before === null) return;
    // 取消＝把語言退回原本的，讓玩家在設定頁重新選（不算問過）
    if (answer === "cancel") {
      await changePreference("language", before);
      return;
    }
    await changePreference("sample_regen_asked", true);
    if (answer === "keep") return;
    const current = chatConfigRef.current;
    if (!current) return;
    setError("");
    try {
      const id = await invoke<string>("create_sample_world", {
        lang: normalizeLang(current.preferences["language"]),
      });
      setWorlds(await invoke<WorldMeta[]>("list_worlds"));
      await enterTable(id, current);
    } catch (reason) {
      setError(String(reason));
    }
  }

  if (!config || !table) {
    return <main className="container">{error && <ErrorNote text={error} />}</main>;
  }

  const tableName = worlds.find((w) => w.id === table)?.name ?? "";
  const stateFields = [
    { key: "time", label: t("stateFieldTime") },
    { key: "place", label: t("stateFieldPlace") },
    { key: "present", label: t("stateFieldPresent") },
    ...Object.keys(tableState)
      .filter((key) => !["time", "place", "present"].includes(key))
      .map((key) => ({ key, label: key })),
  ];
  const stateValue = (key: string) => tableState[key] || t("stateEmptyValue");

  // 換場提醒：粗估目前場景累計字元數，超過門檻就在送出鈕旁小字提醒（不擋操作）
  const sceneChars = events.reduce((sum, event) => sum + event.text.length, 0);
  const sceneTooLong = sceneChars > SCENE_LENGTH_HINT_CHARS;
  // 離開太久＋紀錄夠長才提醒換幕：兩者缺一，換幕都是白花一次摘要錢
  const showAwayHint = awayTooLong && sceneChars > SCENE_AWAY_HINT_MIN_CHARS;

  // 拖曳分隔線調側欄寬度：上限由 CSS max-width 夾住，這裡只擋下限
  function resizeSidebar(next: number) {
    const clamped = Math.min(Math.max(next, SIDEBAR_MIN_WIDTH), window.innerWidth / 2);
    setSidebarWidth(clamped);
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(Math.round(clamped)));
  }

  function startSidebarResize(event: React.PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const onMove = (moveEvent: PointerEvent) => resizeSidebar(moveEvent.clientX);
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  return (
    <div className="app-shell">
      {genTableOpen && (
        <div className="modal-overlay">
          <div className="modal gen-table-modal" role="dialog" aria-modal="true" aria-label={t("genTitle")} onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <strong>{t("genTitle")}</strong>
              <button type="button" className="modal-close" aria-label={t("closeBtn")} disabled={genBusy !== null} onClick={() => setGenTableOpen(false)}>×</button>
            </div>
            <textarea
              rows={4}
              value={genInput}
              placeholder={t("genInputPlaceholder")}
              aria-label={t("genInputPlaceholder")}
              disabled={genBusy !== null}
              onChange={(event) => setGenInput(event.currentTarget.value)}
            />
            <div className="gen-genres">
              {GENRE_KEYS.map((key) => {
                const selected = genGenres.includes(key);
                return (
                  <button
                    key={key}
                    type="button"
                    className={`gen-genre${selected ? " gen-genre-selected" : ""}`}
                    aria-pressed={selected}
                    disabled={genBusy !== null}
                    onClick={() => setGenGenres((current) => selected ? current.filter((genre) => genre !== key) : [...current, key])}
                  >
                    {t(key)}
                  </button>
                );
              })}
            </div>
            <div className="gen-submit-row">
              <button type="button" className="gen-submit" disabled={genBusy !== null || (!genInput.trim() && genGenres.length === 0)} onClick={() => void generateTableOutline()}>
                {genBusy === "outline" ? t("genGenerating") : t("genGenerateBtn")}
              </button>
              <small>{t("genQuotaNote")}</small>
            </div>
            {genOutline && (
              <section className="gen-outline-preview">
                <input
                  value={genOutline.title}
                  disabled={genBusy !== null}
                  onChange={(event) => setGenOutline((current) => current && { ...current, title: event.currentTarget.value })}
                />
                <textarea
                  rows={6}
                  value={genOutline.world}
                  disabled={genBusy !== null}
                  onChange={(event) => setGenOutline((current) => current && { ...current, world: event.currentTarget.value })}
                />
                <h3>{t("genCharListTitle")}</h3>
                <div className="gen-character-list">
                  {genOutline.characters.map((character, index) => (
                    <div className="gen-character-row" key={index}>
                      <input
                        className="gen-character-name"
                        value={character.name}
                        disabled={genBusy !== null}
                        onChange={(event) => setGenOutline((current) => current && {
                          ...current,
                          characters: current.characters.map((item, itemIndex) => itemIndex === index ? { ...item, name: event.currentTarget.value } : item),
                        })}
                      />
                      <textarea
                        rows={2}
                        value={character.tagline}
                        disabled={genBusy !== null}
                        ref={(element) => {
                          if (element) resizeGeneratedCharacterTagline(element);
                        }}
                        onInput={(event) => resizeGeneratedCharacterTagline(event.currentTarget)}
                        onChange={(event) => setGenOutline((current) => current && {
                          ...current,
                          characters: current.characters.map((item, itemIndex) => itemIndex === index ? { ...item, tagline: event.currentTarget.value } : item),
                        })}
                      />
                      <button
                        type="button"
                        className="gen-remove-character"
                        aria-label={t("genRemoveCharacter")}
                        disabled={genBusy !== null}
                        onClick={() => setGenOutline((current) => current && {
                          ...current,
                          characters: current.characters.filter((_, itemIndex) => itemIndex !== index),
                        })}
                      >
                        ×
                      </button>
                    </div>
                  ))}
                </div>
                <button
                  type="button"
                  className="gen-add-character"
                  disabled={genBusy !== null}
                  onClick={() => setGenOutline((current) => current && {
                    ...current,
                    characters: [...current.characters, { name: "", tagline: "" }],
                  })}
                >
                  ＋ {t("genAddCharacter")}
                </button>
                <div className="gen-add-character-ai">
                  <input
                    value={genCharacterHint}
                    placeholder={t("genCharHintPlaceholder")}
                    aria-label={t("genCharHintPlaceholder")}
                    disabled={genBusy !== null}
                    onChange={(event) => setGenCharacterHint(event.currentTarget.value)}
                  />
                  <button
                    type="button"
                    disabled={genBusy !== null}
                    onClick={() => void generateTableCharacter()}
                  >
                    {genBusy === "character" ? t("genCharGenerating") : t("genAddCharacterAI")}
                  </button>
                </div>
              </section>
            )}
            {(genResultRaw !== null || genError) && (
              <section className="gen-result-error" role="alert">
                <p>{genError || t(genResultMessage === "character" ? "genCharParseFail" : "genParseFail")}</p>
                <pre>{genError || genResultRaw}</pre>
              </section>
            )}
            {genOutlineRaw !== null && (
              <div className="gen-result-actions">
                <button type="button" disabled={genBusy !== null} onClick={() => void generateTableOutline()}>{t("genRerollBtn")}</button>
                <button type="button" className="gen-submit" disabled={genBusy !== null || !genOutline || !genOutline.title.trim() || !genOutline.world.trim()} onClick={() => void createGeneratedTable()}>
                  {genBusy === "expand" ? t("genExpanding") : t("genCreateBtn")}
                </button>
              </div>
            )}
          </div>
        </div>
      )}
      <aside className="sidebar" style={{ width: sidebarWidth }}>
        <details
          className="table-section"
          open={tableListOpen}
          onToggle={(event) => {
            const next = event.currentTarget.open;
            setTableListOpen(next);
            localStorage.setItem(TABLE_LIST_OPEN_KEY, String(next));
          }}
        >
          <summary>{t("tableListAria")}</summary>
          <div className="table-section-content">
            <button className="new-table" onClick={newTable} disabled={generating !== null}>
              {t("newTable")}
            </button>
            <button className="gen-table" onClick={() => setGenTableOpen(true)} disabled={generating !== null}>
              {t("genTableBtn")}
            </button>
            <nav className="table-list" aria-label={t("tableListAria")}>
              {worlds.map((w) => (
                <div className="table-row" key={w.id}>
                  {/* 目前這桌再點一次＝改名（切桌沒意義），與主欄標題同一個入口 */}
                  {editingName?.at === "list" && w.id === table ? (
                    renameForm("table-item-input")
                  ) : (
                    <button
                      className={`table-item ${w.id === table ? "table-item-active" : ""}`}
                      title={w.id === table ? t("renameHint") : undefined}
                      onClick={() =>
                        w.id === table
                          ? setEditingName({ at: "list", value: w.name })
                          : switchTable(w.id)
                      }
                    >
                      {w.name}
                    </button>
                  )}
                  <button
                    type="button"
                    className="table-delete"
                    aria-label={t("deleteTableTitle")}
                    title={t("deleteTableTitle")}
                    disabled={generating !== null}
                    onClick={() => void deleteTable(w.id)}
                  >
                    ✕
                  </button>
                </div>
              ))}
            </nav>
          </div>
        </details>
        <section className="character-panel" aria-label={t("castAria")}>
          <div className="character-list">
            {/* GM 卡：與角色卡同款同尺寸同操作（GM 是桌上最重要的一位）——點擊選為發言對象，右下編輯鈕開世界設定＋世界書 */}
            <div
              role="button"
              tabIndex={0}
              className={`tcard tcard-gm ${selectedCard === GM_TARGET ? "tcard-selected" : ""}`}
              title={
                !mainView && speaker === GM_TARGET
                  ? t("gmTargetHintClear")
                  : t("gmTargetHint")
              }
              onClick={() => void selectGm()}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  void selectGm();
                }
              }}
            >
              <span className="tcard-art">
                {gmImage ? (
                  <img className="tcard-image" src={gmImage} alt="" />
                ) : (
                  <img className="gm-book" src={gmBook} alt="" />
                )}
              </span>
              <span className="tcard-body">
                <span className="tcard-name-row">
                  <span className="tcard-plate">GM</span>
                </span>
              </span>
              <button
                type="button"
                className="character-card-edit"
                aria-label={t("worldSummary")}
                title={t("worldSummary")}
                onClick={(event) => {
                  event.stopPropagation();
                  void openWorldEditor();
                }}
              >
                {t("editBtn")}
              </button>
            </div>
            <div
              role="button"
              tabIndex={0}
              className={`tcard tcard-player${playerCard ? "" : " tcard-player-empty"}`}
              title={t(playerCard ? "playerCardHint" : "playerCardEmptyHint")}
              onClick={() => void openPlayerCard()}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  void openPlayerCard();
                }
              }}
            >
              {playerCard ? (
                <>
                  <span className="tcard-art">
                    {playerCard.show_image && playerImage ? (
                      <img className="tcard-image" src={playerImage} alt="" />
                    ) : playerAvatar ? (
                      <img className="avatar-round tcard-avatar" src={playerAvatar} alt="" />
                    ) : (
                      <span aria-hidden="true">{playerCard.avatar}</span>
                    )}
                  </span>
                  <span className="tcard-body">
                    <span className="tcard-name-row">
                      <span className="tcard-plate">{playerCard.name}</span>
                    </span>
                  </span>
                </>
              ) : (
                <span className="tcard-body">{t("playerCardEmpty")}</span>
              )}
            </div>
            {/* 角色卡＝桌遊組件卡：圖窗＋名字 wedge＋檔位寶石（中＝預設檔位，不掛寶石） */}
            {castDrag.order.map((c) => (
              <div
                key={c.id}
                role="button"
                tabIndex={0}
                className={`tcard ${selectedCard === c.id ? "tcard-selected" : ""}${
                  castDrag.draggingKey === c.id ? " row-dragging" : ""
                }`}
                style={{ ["--fac" as string]: c.color }}
                onClick={() => {
                  if (castDrag.justDragged()) return;
                  void selectCard(c.id);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    void selectCard(c.id);
                  }
                }}
                title={`${
                  !mainView && speaker === c.id
                    ? t("castHintClear", { name: c.name })
                    : t("castHint", { name: c.name })
                }｜${t("dragToReorder")}`}
                {...castDrag.rowProps(c)}
              >
                <span className="tcard-art">
                  {c.show_image && characterImages[c.id] ? (
                    <img className="tcard-image" src={characterImages[c.id]} alt="" />
                  ) : characterAvatars[c.id] ? (
                    <img className="avatar-round tcard-avatar" src={characterAvatars[c.id]} alt="" />
                  ) : (
                    <span aria-hidden="true">{c.avatar}</span>
                  )}
                </span>
                <span className="tcard-body">
                  <span className="tcard-name-row">
                    <span className="tcard-plate">{c.name}</span>
                    {c.tier !== "balanced" && (
                      <span className="tcard-gem">{tierLabel(c.tier)}</span>
                    )}
                  </span>
                </span>
                <button
                  type="button"
                  className="character-card-edit"
                  aria-label={t("editCardSummary", { name: c.name })}
                  title={t("editCardSummary", { name: c.name })}
                  onClick={(event) => {
                    event.stopPropagation();
                    void editCard(c.id);
                  }}
                >
                  {t("editBtn")}
                </button>
              </div>
            ))}
          </div>
          {archivedCharacters.length > 0 && (
            <details className="archive-section">
              <summary>{t("archiveSectionTitle")}</summary>
              <div className="archive-list">
                {archivedCharacters.map((character) => {
                  // 沒被玩家手動封存、純粹本幕還沒出場才算自動隱藏；封存優先於自動隱藏顯示
                  const isAutoHidden = !character.archived && character.auto_hidden;
                  return (
                    <div className="archive-row" key={character.id}>
                      <span className="archive-row-name">
                        <span className="archive-row-name-text">{character.name}</span>
                        {isAutoHidden && (
                          <span className="archive-row-badge">{t("autoHiddenBadge")}</span>
                        )}
                      </span>
                      {/* 隱藏卡也要進得了編輯器：轉成世界書條目只能在隱藏狀態下按 */}
                      <button type="button" onClick={() => void editCard(character.id)}>
                        {t("editBtn")}
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          void (isAutoHidden ? restoreAutoHidden(character.id) : restoreCharacter(character.id))
                        }
                      >
                        {t("restoreCharacter")}
                      </button>
                      <button
                        type="button"
                        className="delete-character"
                        onClick={() => void deleteCharacter(character.id)}
                      >
                        {t("deleteCharacter")}
                      </button>
                    </div>
                  );
                })}
              </div>
            </details>
          )}
          {/* 建卡＝直接開空白角色卡編輯器，名字與內容都在那邊填（2026-07-27 使用者拍板） */}
          <div className="character-create">
            <button type="button" onClick={() => void openNewCard()}>
              {t("createCard")}
            </button>
            <button
              type="button"
              title={t("importCardHint")}
              onClick={() => importInputRef.current?.click()}
            >
              {t("importCard")}
            </button>
            <input
              ref={importInputRef}
              type="file"
              accept=".png,.json,image/png,application/json"
              hidden
              onChange={(e) => {
                const file = e.currentTarget.files?.[0];
                e.currentTarget.value = "";
                if (file) void importCharacter(file);
              }}
            />
            {importReceipts.length > 0 && !chattedSinceImport && (
              <button
                type="button"
                title={t("undoLastImportHint")}
                onClick={() => void undoLastImport()}
              >
                {t("undoLastImport")}
              </button>
            )}
          </div>
        </section>
        <div className="sidebar-footer">
          <button className="settings-open" onClick={() => setSettingsOpen("appearance")}>
            ⚙️ {t("settingsBtn")}
          </button>
        </div>
      </aside>

      <div
        className="sidebar-resizer"
        role="separator"
        aria-orientation="vertical"
        aria-label={t("sidebarResizerAria")}
        aria-valuenow={Math.round(sidebarWidth)}
        tabIndex={0}
        onPointerDown={startSidebarResize}
        onKeyDown={(e) => {
          if (e.key === "ArrowLeft") resizeSidebar(sidebarWidth - SIDEBAR_KEY_STEP);
          if (e.key === "ArrowRight") resizeSidebar(sidebarWidth + SIDEBAR_KEY_STEP);
        }}
        onDoubleClick={() => resizeSidebar(SIDEBAR_DEFAULT_WIDTH)}
      />

      <main className="chat-main">
        <header className="chat-header">
          {editingName?.at === "header" ? (
            renameForm("table-title-input")
          ) : (
            <button
              className="table-title"
              title={t("renameHint")}
              onClick={() => setEditingName({ at: "header", value: tableName })}
            >
              {tableName}
            </button>
          )}
          <div className="chat-header-actions">
            {/* 沒有可用殼的桌完全不出現這顆鈕——不是每張卡都帶介面；且只在遊玩畫面（mainView === null）出現 */}
            {mainView === null && cardShellReady && (
              <button type="button" onClick={() => setCardUiOpen(true)}>
                {t("cardInterfaceOpen")}
              </button>
            )}
            <button
              type="button"
              title={t("sceneAdvanceHint")}
              aria-label={t("sceneAdvance")}
              disabled={generating !== null || events.length === 0}
              onClick={advanceScene}
            >
              {t("sceneAdvance")}
            </button>
            <button
              type="button"
              title={t("exportTranscriptHint")}
              aria-label={t("exportTranscript")}
              onClick={exportTranscript}
            >
              {t("exportTranscript")}
            </button>
            {scene > 0 && (
              <button type="button" onClick={() => setActsOpen((open) => !open)}>
                {t("pastScenes", { count: scene })}
              </button>
            )}
          </div>
        </header>

        {mainView === null && (hasStateBar || Object.keys(tableTree).length > 0) && (
        <details
          className="state-bar"
          open={stateBarOpen}
          onToggle={(event) => {
            const next = event.currentTarget.open;
            setStateBarOpen(next);
            localStorage.setItem(STATE_BAR_OPEN_KEY, String(next));
          }}
        >
          <summary>
            <span className="state-bar-title">{t("stateBarTitle")}</span>
            <span className="state-bar-summary">
              {stateValue("time")} ｜ {stateValue("place")} ｜ {t("stateSummaryPresent")}
              {stateValue("present")}
            </span>
          </summary>
          <div className="state-bar-fields">
            {stateFields.map(({ key, label }) => stateLeafRow([key], false, label))}
            {stateTreeNodes(tableTree, [], 0)}
          </div>
        </details>
        )}

        <div className="chat-body">
        {actsOpen && scene > 0 && (
          <div className="acts-flyout">
            <button type="button" className="acts-flyout-hide" onClick={() => setActsOpen(false)}>
              {t("hideActs")}
            </button>
            <div className="acts-flyout-list">
              {Array.from({ length: scene }, (_, n) => n).map((n) => (
                <button
                  key={n}
                  type="button"
                  onClick={() => {
                    setMainView({ kind: "scene", n });
                    setActsOpen(false);
                  }}
                >
                  {sceneDisplayLabel(n)}
                </button>
              ))}
            </div>
          </div>
        )}
        {mainView?.kind === "scene" ? (
          <ActReader
            world={table}
            worldName={tableName}
            scene={mainView.n}
            label={sceneDisplayLabel(mainView.n)}
            onBack={() => setMainView(null)}
            onFork={() => void forkScene(mainView.n)}
          />
        ) : cardView ? (
          <EditPane
            title={
              cardView.kind === "new-character"
                ? t("newCardTitle")
                : cardView.kind === "new-player"
                  ? t("newPlayerCardTitle")
                  : cardView.kind === "player"
                    ? t("editPlayerCardTitle")
                    : t("editCardSummary", { name: metaOf(cardView.id)?.name ?? "" })
            }
          >
            <CardEditor
              world={table}
              characterId={cardView.id}
              isNew={cardView.kind === "new-character" || cardView.kind === "new-player"}
              isPlayer={editingPlayerCard}
              newCardColor={PALETTE[characters.length % PALETTE.length]}
              imageDataUrl={
                editingPlayerCard ? playerImage ?? undefined : characterImages[cardView.id]
              }
              avatarImgUrl={
                editingPlayerCard ? playerAvatar ?? undefined : characterAvatars[cardView.id]
              }
              onImagesChanged={() =>
                editingPlayerCard
                  ? loadPlayerCard(table, playerCard?.id ?? null)
                  : loadCharacterImages(table, characters)
              }
              onSaved={(saved) =>
                void (editingPlayerCard ? finishPlayerCardSaved(saved) : finishCardSaved(saved))
              }
              onArchived={
                cardView.kind === "character"
                  ? () => finishRemoval(cardView.id)
                  : async () => setMainView(null)
              }
              onDeleted={
                cardView.kind === "character"
                  ? () => deleteCharacter(cardView.id)
                  : cardView.kind === "player"
                    ? () => deletePlayerCard(cardView.id)
                    : async () => setMainView(null)
              }
              onBack={() => setMainView(null)}
              leaveGuard={leaveGuard}
              config={config}
              sponsorUnlocked={sponsorUnlocked}
              onPreference={changePreference}
              onOpenAiSettings={() => setSettingsOpen("ai")}
              onConverted={() => finishRemoval(cardView.id)}
            />
          </EditPane>
        ) : mainView?.kind === "world" ? (
          <EditPane title={t("worldSummary")}>
            <WorldEditor
              key={worldEditorRefreshKey}
              world={table}
              worldName={tableName}
              onBack={() => setMainView(null)}
              leaveGuard={leaveGuard}
              convertColor={PALETTE[characters.length % PALETTE.length]}
              onEntryConverted={async () => {
                await refreshCharacters();
              }}
              onRefactorApplied={async () => {
                await refreshCharacters();
                // 重構把原卡拆成一群 NPC：發言對象一律撥回 GM，
                // 不然玩家一開口變成在跟其中一名拆出來的角色對話，回覆完全對不上
                setSpeaker(GM_TARGET);
                // 合併升格可能把某位角色指定為玩家卡（要點 4），跟單條「轉成角色卡」的
                // asPlayer 分支一樣重讀一次，讓側欄玩家卡即時反映。
                const state = await invoke<WorldState>("read_state", { worldId: table });
                await loadPlayerCard(table, state.player_card_id);
                await refreshCardInterfaces(table);
                await refreshRefactorShell(table);
                await refreshTableState();
                await refreshImportReceipts(table);
              }}
            />
          </EditPane>
        ) : (
          <>
            <Onboarding config={config} onSaved={setConfig} />

            <section className="messages" aria-label={t("messagesAria")}>
              {/* 幕書籤：目前這一幕的既有系統標籤（換幕／前幕／單幕匯出同一套資料） */}
              <div className="act-divider">
                <span className="act-tag">{sceneDisplayLabel(scene)}</span>
              </div>
              {events.map((event, index) => {
                if (event.kind === "dialogue" || event.kind === "player") {
                  const meta = metaOf(event.speaker_id);
                  const isPlayer = event.kind === "player";
                  return (
                    <div
                      key={index}
                      className={`message message-${event.kind}`}
                      style={
                        isPlayer ? undefined : { ["--fac" as string]: meta?.color ?? "#888888" }
                      }
                    >
                      <div className="pb-name">
                        <span className="pb-plate">{event.speaker_name}</span>
                      </div>
                      <StoryText text={event.text} />
                    </div>
                  );
                }
                return (
                  <div key={index} className={`message message-${event.kind}`}>
                    <StoryText text={event.text} />
                  </div>
                );
              })}
              {generating !== null && generating.kind === "dialogue" && (
                <div
                  className="message message-dialogue"
                  style={{ ["--fac" as string]: generatingMeta?.color ?? "#888888" }}
                >
                  <div className="pb-name">
                    <span className="pb-plate">{generatingMeta?.name ?? ""}</span>
                  </div>
                  {streamText ? (
                    <span className="text">{streamText}</span>
                  ) : (
                    <span className="typing" aria-label={t("typing", { name: generatingMeta?.name ?? "" })}>
                      <i />
                      <i />
                      <i />
                    </span>
                  )}
                </div>
              )}
              {generating !== null && generating.kind === "narration" && (
                <div className="message message-narration">
                  {narrationStreamText(streamText) ? (
                    <span className="text">{narrationStreamText(streamText)}</span>
                  ) : (
                    <span className="typing" aria-label={t("typing", { name: "GM" })}>
                      <i />
                      <i />
                      <i />
                    </span>
                  )}
                </div>
              )}
              {canRestore && generating === null && (
                <div className="undo-restore">
                  <button type="button" onClick={() => void restoreUndone()}>
                    ↩ {t("undoRestore")}
                  </button>
                </div>
              )}
              {/* 換幕的兩條補救路：只在這一幕還沒開始玩時出現，玩家一發言就自動收掉 */}
              {canUndoScene && (
                <div className="undo-restore">
                  <button
                    type="button"
                    title={t("sceneSummaryRetryHint")}
                    onClick={() => void regenerateSummary()}
                  >
                    ↻ {t("sceneSummaryRetry")}
                  </button>
                  <button
                    type="button"
                    title={t("sceneRevertHint")}
                    onClick={() => void revertScene()}
                  >
                    ↩ {t("sceneRevert")}
                  </button>
                </div>
              )}
              <div ref={bottomRef} />
            </section>

            {/* Composer 改整寬書寫面（ui-overhaul 拍板）：目標晶片只是把「點側欄選發言對象」既有狀態可見化 */}
            <form className="composer" onSubmit={send}>
              {speaker && (
                <div className="composer-opts">
                  <span
                    className="opt-target"
                    title={gmTargeted ? t("gmTargetHint") : t("castHint", { name: targetName })}
                    style={{
                      ["--fac" as string]: gmTargeted ? GM_COLOR : (metaOf(speaker)?.color ?? "#888888"),
                    }}
                  >
                    {gmTargeted ? (
                      gmImage ? (
                        <img className="avatar-round opt-avatar gm-opt-avatar" src={gmImage} alt="" />
                      ) : (
                        <img className="opt-avatar" src={gmBook} alt="" />
                      )
                    ) : characterAvatars[speaker] ? (
                      <img className="avatar-round opt-avatar" src={characterAvatars[speaker]} alt="" />
                    ) : (
                      <span aria-hidden="true">{metaOf(speaker)?.avatar ?? "🎭"}</span>
                    )}
                    {targetName}
                    <button
                      type="button"
                      className="opt-target-clear"
                      aria-label={t("clearTarget")}
                      title={t("clearTarget")}
                      onClick={() => setSpeaker("")}
                    >
                      ✕
                    </button>
                  </span>
                </div>
              )}
              <input
                className="writebox"
                aria-label={t("composerAria")}
                value={input}
                onChange={(e) => setInput(e.currentTarget.value)}
                placeholder={
                  speaker
                    ? t("composerPlaceholder", { name: targetName })
                    : activeCharacters.length === 0
                      ? t("composerNoCharacter")
                      : t("composerNoTarget")
                }
                disabled={(!speaker && activeCharacters.length === 0) || generating !== null}
              />
              {/* 送出擺最左：它跟輸入框是同一件事，右邊那三顆是交給 AI 的動作
                  （2026-07-28 使用者回報：送出在右下容易誤按成「請某某發言」） */}
              <div className="composer-send">
                <div className="composer-primary-action">
                  <button
                    type="submit"
                    disabled={(!speaker && activeCharacters.length === 0) || generating !== null}
                  >
                    {t("send")} ➤
                  </button>
                </div>
                {/* 兩個換幕提醒只顯示一個：離開太久（快取已清）比紀錄長更急，優先出 */}
                {showAwayHint ? (
                  <span className="scene-length-hint">{t("sceneAwayHint")}</span>
                ) : sceneTooLong ? (
                  <span className="scene-length-hint">{t("sceneTooLongHint")}</span>
                ) : null}
                <div className="composer-ai-actions">
                  <button
                    className="undo-last"
                    type="button"
                    onClick={() => void undoLast()}
                    disabled={generating !== null || events.length === 0}
                    title={t("undoLastHint")}
                  >
                    ↩ {t("undoLast")}
                  </button>
                  <button
                    className="request-reply"
                    type="button"
                    onClick={() => void replyFromTarget()}
                    disabled={!speaker || generating !== null}
                    title={`${requestReplyLabel} — ${t("requestReplyHint")}`}
                    aria-label={requestReplyLabel}
                  >
                    <span className="request-reply-label">{requestReplyLabel}</span>
                  </button>
                  <button
                    type="button"
                    onClick={gmNarrate}
                    disabled={generating !== null}
                    title={t("gmNarrateHint")}
                  >
                    {t("gmNarrate")}
                  </button>
                  <button
                    type="button"
                    onClick={gmAdvance}
                    disabled={generating !== null || activeCharacters.length === 0}
                    title={t("gmAdvanceHint")}
                  >
                    {t("gmAdvance")}
                  </button>
                </div>
              </div>
            </form>
          </>
        )}
        </div>
        {error && <ErrorNote text={error} />}
      </main>

      {/* 卡片自帶介面整面取代對話；殼本身已含敘事畫面，不用再疊聊天記錄；且只在遊玩畫面出現——
          切去編輯畫面時 mainView 不再是 null，這裡直接不渲染，覆蓋層就跟著消失，不用另外清 cardUiOpen */}
      {mainView === null && cardUiOpen && cardShellReady && (
        <div className="card-interface-overlay">
          {generating !== null && (
            <div className="card-interface-status" role="status">
              {t("typing", { name: generating.kind === "narration" ? "GM" : (generatingMeta?.name ?? "GM") })}
              <span className="typing">
                <i />
                <i />
                <i />
              </span>
            </div>
          )}
          <div className="card-interface-toolbar">
            <button
              type="button"
              className="modal-close card-interface-close"
              aria-label={t("cardInterfaceClose")}
              onClick={() => setCardUiOpen(false)}
            >
              ✕
            </button>
          </div>
          {/* 單 iframe 直繪：key＝殼指紋，殼一換整支重掛（掛載時 srcdoc 就在，必然載入）。
              殼更新瞬間可能閃一下白，換來顯示的確定性。 */}
          {cardShellDoc !== null && (
            <iframe
              key={cardShellKey}
              className="card-interface-frame"
              sandbox="allow-scripts"
              srcDoc={cardShellDoc}
              title={t("cardInterfaceOpen")}
            />
          )}
        </div>
      )}

      {/* 放整個版面最後：設定視窗永遠疊在其他 modal（含生圖對話框）之上 */}
      {settingsOpen !== false && (
        <SettingsWindow
          config={config}
          onSaved={setConfig}
          onPreference={(key, value) => void changeSettingPreference(key, value)}
          sponsorUnlocked={sponsorUnlocked}
          onSponsorUnlocked={() => setSponsorUnlocked(true)}
          onClose={() => setSettingsOpen(false)}
          initialTab={settingsOpen}
          currentWorld={table}
        />
      )}

      {/* 疊在設定視窗之上：換語言後範例桌要不要重生，一輩子只問這一次 */}
      {regenAsk !== null && (
        <div className="modal-overlay" onClick={() => void answerRegen("cancel")}>
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label={t("sampleRegenTitle")}
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t("sampleRegenTitle")}</h2>
            <p>{t("sampleRegenBody")}</p>
            <div className="ai-gen-footer">
              <button type="button" onClick={() => void answerRegen("cancel")}>
                {t("sampleRegenCancel")}
              </button>
              <button type="button" onClick={() => void answerRegen("keep")}>
                {t("sampleRegenKeep")}
              </button>
              <button type="button" onClick={() => void answerRegen("regen")}>
                {t("sampleRegenConfirm")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 匯入身分框：有名字的卡一律問。直說偵測到哪一種，該身分當主按鈕，另一邊只警告可能玩不動 */}
      {importChoice !== null && (
        <div className="modal-overlay" onClick={() => void answerImportChoice("cancel")}>
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label={t(importChoice.booksFirst ? "importChoiceBookTitle" : "importChoiceCharacterTitle")}
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t(importChoice.booksFirst ? "importChoiceBookTitle" : "importChoiceCharacterTitle")}</h2>
            <p>{t(importChoice.booksFirst ? "importChoiceBookBody" : "importChoiceCharacterBody")}</p>
            <div className="ai-gen-footer">
              <button type="button" onClick={() => void answerImportChoice("cancel")}>
                {t("importChoiceCancel")}
              </button>
              {importChoice.booksFirst ? (
                <>
                  <button type="button" onClick={() => void answerImportChoice("character")}>
                    {t("importChoiceCharacter")}
                  </button>
                  <button type="button" className="ai-gen-submit" onClick={() => void answerImportChoice("worldbook")}>
                    {t("importChoiceWorldbook")}
                  </button>
                </>
              ) : (
                <>
                  <button type="button" onClick={() => void answerImportChoice("worldbook")}>
                    {t("importChoiceWorldbook")}
                  </button>
                  <button type="button" className="ai-gen-submit" onClick={() => void answerImportChoice("character")}>
                    {t("importChoiceCharacter")}
                  </button>
                </>
              )}
            </div>
          </div>
        </div>
      )}

      {/* 第二張卡路由框：桌上已有匯入紀錄才會跳出來。三個選項都給，開新桌是主按鈕；
          第二本世界書換標題與文案（會合成一本），中間那顆改叫「仍要匯入」 */}
      {importRoute !== null && (
        <div className="modal-overlay" onClick={() => void answerImportRoute("cancel")}>
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label={t(importRoute.route === "merge_worldbook" ? "importRouteMergeTitle" : "importRouteAskTitle")}
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t(importRoute.route === "merge_worldbook" ? "importRouteMergeTitle" : "importRouteAskTitle")}</h2>
            <p>{t(importRoute.route === "merge_worldbook" ? "importRouteMergeBody" : "importRouteAskBody")}</p>
            <div className="ai-gen-footer">
              <button type="button" onClick={() => void answerImportRoute("cancel")}>
                {t("importChoiceCancel")}
              </button>
              <button type="button" onClick={() => void answerImportRoute("this_table")}>
                {t(importRoute.route === "merge_worldbook" ? "importRouteMergeAnyway" : "importRouteThisTable")}
              </button>
              <button
                type="button"
                className="ai-gen-submit"
                onClick={() => void answerImportRoute("new_table")}
                disabled={generating !== null}
              >
                {t("importRouteNewTable")}
              </button>
            </div>
          </div>
        </div>
      )}

      {openingChoice !== null && (
        <div className="modal-overlay" onClick={() => setOpeningChoice(null)}>
          <div
            className="modal opening-choice-modal"
            role="dialog"
            aria-modal="true"
            aria-label={t("openingChoiceTitle")}
            onClick={(event) => event.stopPropagation()}
          >
            <div className="modal-header">
              <strong>{t("openingChoiceTitle")}</strong>
              <button type="button" className="modal-close" aria-label={t("closeBtn")} onClick={() => setOpeningChoice(null)}>×</button>
            </div>
            {/* 動作鈕置頂（專案慣例）：全部翻譯放標題正下方，不必展開任何一則就能先按 */}
            <div className="opening-translate-all-row">
              <button
                type="button"
                className="ai-gen-btn"
                title={t("openingTranslateHint")}
                disabled={openingTransAllBusy}
                onClick={() => void translateAllOpenings()}
              >
                {openingTransAllBusy
                  ? t("openingTranslateAllProgress", {
                      done: openingChoice.filter((_, index) => openingTransState[index] === "done" || openingTransState[index] === "error")
                        .length,
                      total: openingChoice.length,
                    })
                  : `✨ ${t("openingTranslateAllBtn")}`}
              </button>
            </div>
            <p>{t("openingLineAsk")}</p>
            <div className="opening-choice-list">
              {openingChoice.map((opening, index) => {
                // 點列只展開全文，貼出的鈕在框外底部——開場白動輒上千字，按鈕若跟在全文後面
                // 得整段捲到底才按得到，而滿是標記的開場白根本沒必要逐字看完
                const expanded = openingExpanded === index;
                const transState = openingTransState[index];
                return (
                  <div className="opening-choice-item" key={index}>
                    <button
                      type="button"
                      className="opening-choice-head"
                      aria-expanded={expanded}
                      onClick={() => setOpeningExpanded(expanded ? null : index)}
                    >
                      <strong>{t("openingChoiceItem", { n: index + 1 })}</strong>
                      {transState === "translating" && <span className="opening-trans-status">{t("openingTranslating")}</span>}
                      {transState === "error" && (
                        <span className="opening-trans-status opening-trans-error" title={t("openingTranslateFailed")}>
                          ⚠
                        </span>
                      )}
                      <span>{expanded ? "" : openingPreview(opening)}</span>
                    </button>
                    {expanded && (
                      <div className="opening-choice-full">
                        <StoryText text={opening} />
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
            <div className="ai-gen-footer">
              {openingExpanded !== null && openingChoice[openingExpanded] !== undefined && (
                <>
                  <button
                    type="button"
                    className="footer-lead"
                    onClick={() => void postOpening(openingChoice[openingExpanded])}
                  >
                    {t("openingLineOk")}
                  </button>
                  <button
                    type="button"
                    className="ai-gen-btn"
                    title={t("openingTranslateHint")}
                    disabled={openingTransState[openingExpanded] === "translating"}
                    onClick={() => void postTranslatedOpening(openingExpanded)}
                  >
                    {openingTransState[openingExpanded] === "translating" ? t("openingTranslating") : `✨ ${t("openingTranslatePostBtn")}`}
                  </button>
                </>
              )}
              <button type="button" onClick={() => setOpeningChoice(null)}>{t("openingLineCancel")}</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
