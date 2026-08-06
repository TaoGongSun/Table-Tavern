import { FormEvent, Fragment, PointerEvent as ReactPointerEvent, useEffect, useMemo, useRef, useState } from "react";
import Cropper, { Area } from "react-easy-crop";
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { confirm, message as showMessage, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { detectLang, Lang, LANGUAGE_OPTIONS, normalizeLang, setLang, t } from "./i18n";
import { renderStoryMarkdown } from "./story-markdown";
import { buildShellDocument, CardInterface, findShell } from "./interface-card";
import { decideImportRoute } from "./import-routing";
import taoIcon from "./assets/tao-icon.png";
import gmBook from "./assets/gm-book.png";
import "./App.css";

const KOFI_URL = "https://ko-fi.com/s/027754730c";
const GALLERY_PAGE_SIZE = 12;
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

type Tier = "best" | "balanced" | "fast";

interface CharacterMeta {
  id: string;
  name: string;
  color: string;
  avatar: string;
  tier: Tier;
  show_image: boolean;
  archived: boolean;
}

interface CharacterCard extends CharacterMeta {
  public_md: string;
  private_md: string;
  gen_prompt: string;
}

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
  removed_entries: number;
  kept_entries: number;
  renamed_back: boolean;
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
  return <span className="text rendered" dangerouslySetInnerHTML={{ __html: html }} />;
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
  state: {
    table: Record<string, string>;
    tree: Record<string, StateNode>;
    // 全量桌的跳動警示：路徑（點分）→ 顯示標記（"+40"／"-80"），增量桌一律是空物件
    jumps?: Record<string, string>;
  };
}

// 狀態樹節點：葉子是值，分支是子節點（對應後端 StateNode 的 untagged 序列化）
type StateNode = string | { [key: string]: StateNode };

// 路徑指到的葉子值；中途撞到分支或缺節點都當空字串（面板只讀，取不到就是沒東西可改）
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

interface AppConfig {
  api_keys: Record<string, string>;
  tier_models: Record<string, string>;
  preferences: Record<string, unknown>;
}

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

// 檔位只是三個插槽，UI 以品質高低命名；內部 key（卡片 frontmatter／config.json）維持不變
const TIER_LABEL_KEYS = {
  best: "tierBest",
  balanced: "tierBalanced",
  fast: "tierFast",
} as const;
const tierLabel = (tier: keyof typeof TIER_LABEL_KEYS) => t(TIER_LABEL_KEYS[tier]);

const PALETTE = ["#e07a5f", "#3d84a8", "#81b29a", "#f2a541", "#9b5de5", "#e56399"];

// 裁切完成的圖：bytes 給後端存檔、url 給畫面預覽（按儲存前只活在記憶體裡）
type DraftImage = { bytes: number[]; url: string };

// 角色圖示快捷選項；輸入框沒限制在這幾個，系統 emoji 鍵盤打什麼都行
const AVATAR_EMOJIS = ["🎭", "🧙", "🗡️", "🏹", "🛡️", "🐺", "🦊", "🐉", "👑", "💀", "🌙", "🕯️"];
const DEFAULT_AVATAR = "🎭";
const AVATAR_MAX_CHARS = 4;
// GM 點到玩家時後端回這個代號（transport.rs 的 PLAYER_SENTINEL），收到就把發言權交回給玩家
const PLAYER_SENTINEL = "__PLAYER__";
// 發言對象是 GM 時 speaker 存這個代號（純前端狀態，不會寫進紀錄）；GM 以旁白回應
const GM_TARGET = "__GM__";
// GM 卡的銅金色：發言對象晶片沿用書皮的 --fac，與角色卡的陣營色區隔
const GM_COLOR = "#8a6a3c";

// 以「看得到的字元」為單位截斷：input 的 maxLength 算的是 UTF-16 單元，
// 一顆 🗡️ 就佔 3 個，拿來限長會讓 emoji 只打得下一顆。
function clampChars(value: string, max: number) {
  const chars =
    typeof Intl.Segmenter === "function"
      ? Array.from(new Intl.Segmenter().segment(value), (unit) => unit.segment)
      : Array.from(value);
  return chars.slice(0, max).join("");
}

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

interface CliInfo {
  id: string;
  path: string;
  version: string;
}

// 偵測結果跨畫面／跨設定頁開關快取：null＝本次啟動還沒偵測過。
// 設定頁重開時先吃上次結果，不再讓使用者重看一次「偵測中」。
let cliCache: CliInfo[] | null = null;

async function detectClis(): Promise<CliInfo[]> {
  const detected = await invoke<CliInfo[]>("detect_clis");
  cliCache = detected;
  return detected;
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

const CLI_LABELS: Record<string, string> = {
  claude: "Claude Code CLI",
  codex: "Codex CLI",
  // 引擎是 Google Antigravity CLI，但一般使用者只認識 Gemini 這個名字（2026-07-25 拍板）
  agy: "Gemini CLI",
  grok: "Grok CLI",
};

// Claude Code CLI 只輸出文字，沒有生圖工具：選到它就直說，不要讓玩家等一輪才拿到失敗訊息
const NO_IMAGE_CLIS = ["claude"];

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

// AI 失敗訊息分流：各家 CLI／API 的錯誤格式不一，只認額度與未登入兩類最痛的，
// 認不出來回 null 走原本的通用文案——誤判比不判更糟（.ai/tasks/ai-error-messages.md）
const QUOTA_ERROR = /usage limit|quota|out of credit|insufficient[ _]credit|insufficient_quota|rate.?limit|resource_exhausted|too many requests|\b402\b|\b429\b/i;
const AUTH_ERROR = /not logged in|not authenticated|unauthorized|authentication|api[ _]key|credential|expired token|\b401\b/i;
const REFUSAL_ERROR =
  /content polic|guideline|safety system|can'?t (create|generate|make|help)|cannot (create|generate|make)|won'?t (create|generate)|unable to (create|generate)|declin|無法(生成|產生|製作)|不能生成|拒絕|违反|違反/i;

function explainAiError(raw: string): "errQuota" | "errAuth" | "errNoImage" | "errRefused" | null {
  // REFUSED／NO_IMAGE 是生圖 prompt 跟 CLI 約好的暗號：不肯生這一張（內容規範）
  // 與根本生不出圖（多半是生圖額度或方案），玩家的下一步不同
  if (raw.includes("REFUSED")) return "errRefused";
  if (raw.includes("NO_IMAGE")) return "errNoImage";
  if (QUOTA_ERROR.test(raw)) return "errQuota";
  if (AUTH_ERROR.test(raw)) return "errAuth";
  // 模型沒照暗號回時的保底：拒絕的原話多半帶這些字樣
  if (REFUSAL_ERROR.test(raw)) return "errRefused";
  return null;
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
  const [clis, setClis] = useState<CliInfo[] | null>(cliCache);
  const [models, setModels] = useState<{ id: string; name: string }[]>([]);
  const [cliCatalogs, setCliCatalogs] = useState<Record<string, { id: string; label: string }[]>>({});
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
    // CLI 模型下拉目錄：讀各 CLI 本機快取（後端 list_cli_models）
    for (const id of ["claude", "codex", "agy", "grok"]) {
      invoke<{ id: string; label: string }[]>("list_cli_models", { cli: id })
        .then((options) => setCliCatalogs((prev) => ({ ...prev, [id]: options })))
        .catch(() => {});
    }
    // OpenRouter 公開模型清單（免 key）；拿不到就退化成純手動輸入
    fetch("https://openrouter.ai/api/v1/models")
      .then((res) => res.json())
      .then((body: { data?: { id?: string; name?: string }[] }) =>
        setModels((body.data ?? []).flatMap((m) => (m.id ? [{ id: m.id, name: m.name ?? m.id }] : []))),
      )
      .catch(() => {});
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
          void detectClis().then(setClis).catch(() => {});
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
      void detectClis().then(setClis).catch(() => {});
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
              {models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name}
                </option>
              ))}
            </datalist>
          </>
        ) : (
          <>
            {(["best", "balanced", "fast"] as const).map((tier) => {
              const key = `${transport}:${tier}`;
              const value = tierModels[key] ?? "";
              const catalog = cliCatalogs[transport] ?? [];
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

// 世界書 v1：一份只進 GM 上下文的 world.md（NewPlan §7.0）
function WorldEditor({
  world,
  onBack,
  leaveGuard,
  convertColor,
  hasPlayerCard,
  onEntryConverted,
}: {
  world: string;
  onBack: () => void;
  /** 側欄要離開世界設定時先問過這裡（未儲存確認與返回鈕同一條） */
  leaveGuard: { current: (() => Promise<boolean>) | null };
  convertColor: string;
  hasPlayerCard: boolean;
  onEntryConverted: (asPlayer: boolean) => Promise<void>;
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
    invoke<CharacterMeta[]>("list_characters", { worldId: world })
      .then((cast) => setCharacters(cast.filter((character) => !character.archived)))
      .catch((reason) => setWorldbookMessage(String(reason)));
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
    if (!target) return;
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

  async function convertEntryToCharacter() {
    if (!draft || draft.uid === null) return;
    setWorldbookMessage("");
    try {
      const asPlayer =
        !hasPlayerCard && /\{\{\s*user\s*\}\}/i.test(draft.content)
          ? await confirm(t("convertEntryPersonaAsk"), {
              title: t("convertEntryToCard"),
              kind: "info",
              okLabel: t("convertEntryPersonaOk"),
              cancelLabel: t("convertEntryPersonaCancel"),
            })
          : false;
      const meta = await invoke<CharacterMeta>("worldbook_entry_to_character", {
        worldId: world,
        uid: draft.uid,
        color: convertColor,
        asPlayer,
      });
      setDraft(null);
      await refreshWorldbook();
      setWorldbookMessage(t("convertEntryDone", { name: meta.name }));
      await onEntryConverted(asPlayer);
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
                    <label className="mechanism-ledger-toggle">
                      <input
                        type="checkbox"
                        checked={entry.sent}
                        onChange={() => void toggleLedgerEntry(entry)}
                      />
                      {t("ledgerSendRaw")}
                    </label>
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
                  </div>
                </div>
                <div className="worldbook-row-actions">
                  <button type="button" onClick={() => editEntry(entry)}>
                    {t("editBtn")}
                  </button>
                  <button type="button" onClick={() => void deleteEntry(entry)}>
                    {t("worldbookDelete")}
                  </button>
                </div>
              </div>
              ),
            )}
          </div>
        )}
        {draft && draft.uid === null && entryForm}
        {worldbookMessage && <p role="status">{worldbookMessage}</p>}
      </section>
    </>
  );
}

function CropDialog({
  title,
  src,
  aspect,
  cropShape,
  onConfirm,
  onCancel,
}: {
  title: string;
  src: string;
  aspect: number;
  cropShape: "rect" | "round";
  onConfirm: (image: DraftImage) => Promise<void>;
  onCancel: () => void;
}) {
  const [crop, setCrop] = useState({ x: 0, y: 0 });
  const [zoom, setZoom] = useState(1);
  const [croppedAreaPixels, setCroppedAreaPixels] = useState<Area | null>(null);
  const [message, setMessage] = useState("");

  async function confirmCrop() {
    if (!croppedAreaPixels) return;
    setMessage("");
    try {
      const image = new Image();
      await new Promise<void>((resolve, reject) => {
        image.onload = () => resolve();
        image.onerror = () => reject(new Error("Unable to load image"));
        image.src = src;
      });
      const size = cropShape === "round" ? 256 : Math.min(Math.round(croppedAreaPixels.width), 1024);
      const height =
        cropShape === "round"
          ? 256
          : Math.max(1, Math.round((croppedAreaPixels.height / croppedAreaPixels.width) * size));
      const canvas = document.createElement("canvas");
      canvas.width = size;
      canvas.height = height;
      const context = canvas.getContext("2d");
      if (!context) throw new Error("Unable to create image canvas");
      // 頭像存正方形原樣，圓形與黑框由 CSS 畫（拍板規格），canvas 不做圓形裁切
      context.drawImage(
        image,
        croppedAreaPixels.x,
        croppedAreaPixels.y,
        croppedAreaPixels.width,
        croppedAreaPixels.height,
        0,
        0,
        size,
        height,
      );
      const blob = await new Promise<Blob>((resolve, reject) => {
        canvas.toBlob((result) => (result ? resolve(result) : reject(new Error("Unable to crop image"))), "image/png");
      });
      // bytes 給存檔用、url 給暫存預覽用（圖像按儲存才落地）
      await onConfirm({
        bytes: Array.from(new Uint8Array(await blob.arrayBuffer())),
        url: canvas.toDataURL("image/png"),
      });
      onCancel();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal" role="dialog" aria-modal="true" aria-label={title} onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <strong>{title}</strong>
          <button type="button" className="modal-close" aria-label={t("closeBtn")} onClick={onCancel}>×</button>
        </div>
        <div className="crop-area">
          <Cropper
            image={src}
            crop={crop}
            zoom={zoom}
            aspect={aspect}
            cropShape={cropShape}
            onCropChange={setCrop}
            onZoomChange={setZoom}
            onCropComplete={(_, area) => setCroppedAreaPixels(area)}
          />
        </div>
        <label className="crop-zoom">
          {t("zoomLabel")}
          <input type="range" min={1} max={4} step={0.05} value={zoom} onChange={(event) => setZoom(Number(event.currentTarget.value))} />
        </label>
        <div className="row">
          <button type="button" onClick={() => void confirmCrop()}>{t("cropConfirm")}</button>
          <button type="button" onClick={onCancel}>{t("cropCancel")}</button>
          {message && <span role="alert">{message}</span>}
        </div>
      </div>
    </div>
  );
}

function CardEditor({
  world,
  characterId,
  isNew,
  newCardColor,
  imageDataUrl,
  avatarImgUrl,
  onImagesChanged,
  onSaved,
  onArchived,
  onDeleted,
  onBack,
  leaveGuard,
  config,
  sponsorUnlocked,
  onPreference,
  onOpenAiSettings,
  isPlayer = false,
  onConverted,
}: {
  world: string;
  /** 開編輯器前已由 new_id 拿好，草稿期生圖與存檔用同一個 id */
  characterId: string;
  /** true＝建新卡的空白草稿，尚未寫入過任何檔案 */
  isNew: boolean;
  /** 側欄要離開這張卡時先問過這裡（未儲存確認與返回鈕同一條） */
  leaveGuard: { current: (() => Promise<boolean>) | null };
  newCardColor: string;
  imageDataUrl?: string;
  avatarImgUrl?: string;
  onImagesChanged: () => Promise<void>;
  onBack: () => void;
  onSaved: (id: string) => void;
  onArchived: () => Promise<void>;
  onDeleted: () => Promise<void>;
  config: AppConfig;
  sponsorUnlocked: boolean;
  onPreference: (key: string, value: unknown) => Promise<void>;
  onOpenAiSettings: () => void;
  isPlayer?: boolean;
  onConverted: () => Promise<void>;
}) {
  const [card, setCard] = useState<CharacterCard | null>(null);
  const [savedCardJson, setSavedCardJson] = useState("");
  // 圖像操作一律暫存，按儲存才落地（2026-07-27 使用者拍板）：
  // undefined＝沒動過（沿用 props 的已存檔圖）、null＝已標記移除、物件＝待存的新圖
  const [draftImage, setDraftImage] = useState<DraftImage | null | undefined>(undefined);
  const [draftAvatar, setDraftAvatar] = useState<DraftImage | null | undefined>(undefined);
  const [message, setMessage] = useState("");
  const [pendingImage, setPendingImage] = useState<string | null>(null);
  const [croppingAvatar, setCroppingAvatar] = useState(false);
  const [lightboxOpen, setLightboxOpen] = useState(false);
  const [aiGenOpen, setAiGenOpen] = useState(false);
  const [aiGenLockedOpen, setAiGenLockedOpen] = useState(false);
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiSource, setAiSource] = useState("api");
  const [aiFraming, setAiFraming] = useState("full");
  const [aiClis, setAiClis] = useState<CliInfo[]>([]);
  const [aiGenerating, setAiGenerating] = useState(false);
  const [aiGenError, setAiGenError] = useState("");
  const [galleryFiles, setGalleryFiles] = useState<string[]>([]);
  const [galleryImages, setGalleryImages] = useState<Record<string, string>>({});
  const [galleryLoaded, setGalleryLoaded] = useState(0);
  // 存檔前判斷「有沒有改名」用；新卡是空字串（第一次存檔不算改名）
  const [originalName, setOriginalName] = useState("");

  useEffect(() => {
    setMessage("");
    setDraftImage(undefined);
    setDraftAvatar(undefined);
    if (isNew) {
      const blank: CharacterCard = {
        id: characterId,
        name: "",
        color: newCardColor,
        avatar: DEFAULT_AVATAR,
        tier: "balanced",
        show_image: true,
        archived: false,
        public_md: "",
        private_md: "",
        gen_prompt: "",
      };
      setCard(blank);
      setSavedCardJson(JSON.stringify(blank));
      setOriginalName("");
      return;
    }
    invoke<CharacterCard>("read_character", { worldId: world, characterId })
      .then((loaded) => {
        setCard(loaded);
        setSavedCardJson(JSON.stringify(loaded));
        setOriginalName(loaded.name);
      })
      .catch((reason) => setMessage(String(reason)));
  }, [world, characterId, isNew, newCardColor]);

  const trialsUsed = Number(config.preferences["ai_image_trials_used"] ?? 0);
  const sourceOptions = ["api", ...aiClis.map((cli) => cli.id)];
  const sourceCannotGenerate = NO_IMAGE_CLIS.includes(aiSource);

  async function loadGalleryPage(files: string[], start: number) {
    const page = files.slice(start, start + GALLERY_PAGE_SIZE);
    const images = await Promise.all(page.map(async (file) => [file, await invoke<string>("read_gallery_image", { worldId: world, characterId, file })] as const));
    setGalleryImages((current) => ({ ...current, ...Object.fromEntries(images) }));
    setGalleryLoaded(Math.min(start + page.length, files.length));
  }

  async function refreshGallery() {
    const files = await invoke<string[]>("list_gallery_images", { worldId: world, characterId });
    setGalleryFiles(files);
    setGalleryImages({});
    setGalleryLoaded(0);
    await loadGalleryPage(files, 0);
  }

  function openAiGenerator() {
    if (!sponsorUnlocked && trialsUsed >= 3) {
      setAiGenLockedOpen(true);
      return;
    }
    const savedSource = String(config.preferences["image_source"] ?? "");
    // 聊天用的來源不一定會生圖（例如 claude），跟隨不到就退回 API，玩家一打開就是能按的狀態
    const transport = String(config.preferences["transport"] ?? "api");
    const fallback = NO_IMAGE_CLIS.includes(transport) ? "api" : transport;
    void detectClis()
      .then((detected) => {
        setAiClis(detected);
        const detectedSources = ["api", ...detected.map((cli) => cli.id)];
        setAiSource(detectedSources.includes(savedSource) ? savedSource : fallback);
      })
      .catch(() => {
        setAiClis([]);
        setAiSource(savedSource === "api" ? savedSource : fallback);
      });
    setAiPrompt(card?.gen_prompt ?? "");
    setAiFraming(config.preferences["image_framing"] === "half" ? "half" : "full");
    setAiGenError("");
    setAiGenOpen(true);
    void refreshGallery().catch(() => {
      setGalleryFiles([]);
      setGalleryImages({});
      setGalleryLoaded(0);
    });
  }

  async function generateImage() {
    setAiGenerating(true);
    setAiGenError("");
    try {
      await invoke<string>("generate_character_image", {
        worldId: world,
        characterId,
        name: card?.name.trim() ?? "",
        description: card?.public_md ?? "",
        extraPrompt: aiPrompt,
        source: aiSource,
        framing: aiFraming,
      });
      // 追加描寫記進草稿，跟其他欄位一起等按儲存才落地
      setCard((current) => (current ? { ...current, gen_prompt: aiPrompt } : current));
      await refreshGallery();
      await onPreference("image_source", aiSource);
      await onPreference("image_framing", aiFraming);
      if (!sponsorUnlocked) await onPreference("ai_image_trials_used", trialsUsed + 1);
    } catch (reason) {
      setAiGenError(String(reason));
    } finally {
      setAiGenerating(false);
    }
  }

  async function deleteGalleryImage(file: string) {
    const accepted = await confirm(t("aiGalleryDeleteConfirm"), { title: t("aiGalleryDeleteTitle"), kind: "warning" });
    if (!accepted) return;
    await invoke("delete_gallery_image", { worldId: world, characterId, file });
    setGalleryFiles((current) => current.filter((item) => item !== file));
    setGalleryImages((current) => {
      const { [file]: _, ...remaining } = current;
      return remaining;
    });
    setGalleryLoaded((current) => Math.max(0, current - (galleryImages[file] ? 1 : 0)));
  }

  if (!card) return message ? <p role="alert">{message}</p> : null;

  const shownImage = draftImage === undefined ? imageDataUrl : draftImage?.url;
  const shownAvatar = draftAvatar === undefined ? avatarImgUrl : draftAvatar?.url;
  const aiGenBlocked = !card.name.trim() || !card.public_md.trim();
  const unsavedCount =
    (JSON.stringify(card) !== savedCardJson ? 1 : 0) +
    (draftImage !== undefined ? 1 : 0) +
    (draftAvatar !== undefined ? 1 : 0);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    if (!card) return;
    const target = card.name.trim();
    if (!target) {
      setMessage(t("nameRequiredError"));
      return;
    }
    // 圖示清空就回預設，免得沒圖也沒 emoji 的空白角色
    const saved: CharacterCard = {
      ...card,
      name: target,
      avatar: card.avatar.trim() || DEFAULT_AVATAR,
      private_md: isPlayer ? "" : card.private_md,
      tier: isPlayer ? "balanced" : card.tier,
    };
    // 改名只換之後的顯示名稱（id 定址不受影響），欄位下的說明太容易看漏，儲存前再提醒一次
    const renaming = !isNew && target !== originalName;
    if (
      renaming &&
      !(await confirm(t("renameConfirm", { from: originalName, to: target }), {
        title: t("renameConfirmTitle"),
        kind: "warning",
      }))
    ) {
      return;
    }
    try {
      await invoke("write_character", { worldId: world, card: saved });
      if (draftImage === null) await invoke("delete_character_image", { worldId: world, characterId });
      else if (draftImage) await invoke("save_character_image", { worldId: world, characterId, data: draftImage.bytes });
      if (draftAvatar === null) await invoke("delete_character_avatar", { worldId: world, characterId });
      else if (draftAvatar) await invoke("save_character_avatar", { worldId: world, characterId, data: draftAvatar.bytes });
      setDraftImage(undefined);
      setDraftAvatar(undefined);
      await onImagesChanged();
      setCard(saved);
      setSavedCardJson(JSON.stringify(saved));
      setOriginalName(target);
      setMessage(t("saved"));
      onSaved(characterId);
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function confirmLeave() {
    if (unsavedCount === 0) return true;
    return await confirm(t("unsavedLeaveConfirm", { n: unsavedCount }), {
      title: t("unsavedLeaveTitle"),
      kind: "warning",
    });
  }
  // 側欄切換編輯對象時走的是同一條確認；每次 render 掛上，閉包才拿得到最新的 unsavedCount
  leaveGuard.current = confirmLeave;

  async function handleBack() {
    if (await confirmLeave()) onBack();
  }

  // 同一顆鈕雙向切換：隱藏區進來的卡按它就是還原，免得編輯器裡出現按了沒意義的「隱藏角色」
  async function toggleArchived() {
    setMessage("");
    try {
      await invoke("set_character_archived", {
        worldId: world,
        characterId,
        archived: card?.archived !== true,
      });
      await onArchived();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  // 匯出成 SillyTavern 角色卡：內容取自已存檔的那份，所以草稿沒存完先擋下
  async function exportCard() {
    setMessage("");
    if (!card) return;
    if (unsavedCount > 0) {
      setMessage(t("exportCardNeedsSave"));
      return;
    }
    try {
      const path = await saveDialog({
        defaultPath: `${card.name.trim() || "card"}.png`,
        filters: [
          { name: t("exportCardPng"), extensions: ["png"] },
          { name: t("exportCardJson"), extensions: ["json"] },
        ],
      });
      if (!path) return;
      await invoke("export_character", { worldId: world, characterId, path });
      await revealItemInDir(path);
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function convertCardToWorldbookEntry() {
    setMessage("");
    if (!card) return;
    if (unsavedCount > 0) {
      await showMessage(t("convertCardUnsaved"), { title: t("convertCardToEntry") });
      return;
    }
    if (card.archived === false) {
      await showMessage(t("convertCardInUse"), { title: t("convertCardToEntry") });
      return;
    }
    const accepted = await confirm(t("convertCardConfirm"), {
      title: t("convertCardToEntry"),
      kind: "warning",
    });
    if (!accepted) return;
    try {
      await invoke("character_to_worldbook_entry", { worldId: world, characterId });
      await showMessage(t("convertCardDone"));
      await onConverted();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  function chooseImage(file: File) {
    const reader = new FileReader();
    reader.onload = () => setPendingImage(typeof reader.result === "string" ? reader.result : null);
    reader.onerror = () => setMessage(String(reader.error));
    reader.readAsDataURL(file);
  }

  // 移除圖片／頭像都會讓卡片退回下一層顯示，先問一聲（2026-07-27 使用者回饋）
  async function removeImage() {
    const accepted = await confirm(t("removeImageConfirm"), {
      title: t("removeImageTitle"),
      kind: "warning",
    });
    if (accepted) setDraftImage(null);
  }

  async function removeAvatar() {
    const accepted = await confirm(t("removeAvatarConfirm"), {
      title: t("removeAvatarTitle"),
      kind: "warning",
    });
    if (accepted) setDraftAvatar(null);
  }

  return (
    <form onSubmit={save} className="settings-form">
      {/* 頂部切兩塊：左邊是這張卡的動作（返回獨立成第二列貼齊儲存下方，按鈕變多後夾在刪除
          旁邊很難找），右邊是圖片與它的操作鈕；打字欄位維持全寬在下方（2026-07-28 使用者拍板） */}
      <div className="card-editor-top">
        <div className="card-editor-actions">
          <div className="row">
            <button type="submit">{t("saveCard")}</button>
            {!isNew && (
              <>
                <button type="button" title={t("exportCardHint")} onClick={() => void exportCard()}>
                  {t("exportCard")}
                </button>
                {!isPlayer && (
                  <button type="button" onClick={() => void convertCardToWorldbookEntry()}>
                    {t("convertCardToEntry")}
                  </button>
                )}
                {!isPlayer && (
                  <button
                    type="button"
                    className="archive-button"
                    onClick={() => void toggleArchived()}
                  >
                    {card?.archived === true ? t("restoreCharacter") : t("archiveCharacter")}
                  </button>
                )}
                <button type="button" className="delete-character" onClick={() => void onDeleted()}>
                  {t("deleteCharacter")}
                </button>
              </>
            )}
          </div>
          <div className="row">
            <button type="button" onClick={() => void handleBack()}>
              {t("backToNow")}
            </button>
          </div>
          {message && <span>{message}</span>}
          {unsavedCount > 0 && (
            <span className="unsaved-hint" role="status">
              {t("unsavedChanges", { n: unsavedCount })}
            </span>
          )}
        </div>
        <div className="card-editor-media">
          <div className="card-editor-avatar">
            {shownImage ? (
              <button
                type="button"
                className="card-editor-image-zoom"
                aria-label={t("viewImageLabel")}
                title={t("viewImageLabel")}
                onClick={() => setLightboxOpen(true)}
              >
                <img className="card-editor-image" src={shownImage} alt="" />
              </button>
            ) : shownAvatar ? (
              <img className="avatar-round card-editor-avatar-round" src={shownAvatar} alt="" />
            ) : (
              <span className="card-editor-avatar-emoji" style={{ ["--ring" as string]: card.color }}>
                {card.avatar}
              </span>
            )}
          </div>
          <div className="row">
            <button type="button" onClick={() => document.getElementById(`character-image-${characterId}`)?.click()}>
              {t(shownImage ? "replaceImageBtn" : "addImageBtn")}
            </button>
            {/* 名字給圖庫資料夾用、公開設定進提示詞；欄位沒填就生不出像樣的圖，故先鎖住。
                提示掛在外層 span：disabled 的按鈕不收滑鼠事件，title 掛上去不會浮出來 */}
            <span className="hint-wrap" data-hint={aiGenBlocked ? t("aiGenNeedsContent") : undefined}>
              <button
                type="button"
                className="ai-gen-btn"
                disabled={aiGenBlocked}
                onClick={openAiGenerator}
              >
                ✨ {t("aiGenBtn")}
              </button>
            </span>
            {shownImage && (
              <>
                <button type="button" onClick={() => void removeImage()}>{t("removeImageBtn")}</button>
                <button type="button" onClick={() => setCroppingAvatar(true)}>{t("makeAvatarBtn")}</button>
              </>
            )}
            {shownAvatar && <button type="button" onClick={() => void removeAvatar()}>{t("removeAvatarBtn")}</button>}
            <input
              id={`character-image-${characterId}`}
              type="file"
              accept="image/png,image/jpeg,image/webp"
              hidden
              onChange={(event) => {
                const file = event.currentTarget.files?.[0];
                event.currentTarget.value = "";
                if (file) chooseImage(file);
              }}
            />
          </div>
        </div>
      </div>
      <label>
        {t(isPlayer ? "playerNameLabel" : "nameLabel")}
        <input
          value={card.name}
          placeholder={t(isPlayer ? "playerNamePlaceholder" : "newCharacterPlaceholder")}
          onChange={(e) => setCard({ ...card, name: e.currentTarget.value })}
        />
      </label>
      {/* 改名只換之後的顯示名稱，已送出的對話仍顯示舊名（2026-07-27 拍板） */}
      {!isNew && card.name.trim() !== originalName && (
        <p className="field-note" role="note">
          {t("renameNote")}
        </p>
      )}
      {/* emoji 只在沒有圖可顯示時才會用到：有頭像、或有大圖且開關開著，這一欄就沒意義（2026-07-28 使用者拍板） */}
      {!shownAvatar && !(shownImage && card.show_image) && (
        <label>
          {t("avatarEmojiLabel")}
          <div className="emoji-row">
            <input
              className="emoji-input"
              value={card.avatar}
              onChange={(e) =>
                setCard({
                  ...card,
                  avatar: clampChars(e.currentTarget.value.replace(/\s/g, ""), AVATAR_MAX_CHARS),
                })
              }
            />
            {AVATAR_EMOJIS.map((emoji) => (
              <button
                key={emoji}
                type="button"
                className="emoji-preset"
                aria-pressed={card.avatar === emoji}
                onClick={() => setCard({ ...card, avatar: emoji })}
              >
                {emoji}
              </button>
            ))}
          </div>
        </label>
      )}
      <label>
        {t(isPlayer ? "playerPublicLabel" : "publicLabel")}
        <textarea
          rows={4}
          value={card.public_md}
          onChange={(e) => setCard({ ...card, public_md: e.currentTarget.value })}
        />
      </label>
      {!isPlayer && (
        <label>
          {t("privateLabel")}
          <textarea
            rows={4}
            value={card.private_md}
            onChange={(e) => setCard({ ...card, private_md: e.currentTarget.value })}
          />
        </label>
      )}
      {shownImage && (
        <label className="inline">
          <input
            type="checkbox"
            checked={card.show_image}
            onChange={(e) => setCard({ ...card, show_image: e.currentTarget.checked })}
          />
          {t("showImageLabel")}
        </label>
      )}
      {!isPlayer && (
        <label>
          {t("tierLabel")}
          <select
            value={card.tier}
            onChange={(e) => setCard({ ...card, tier: e.currentTarget.value as Tier })}
          >
            {(["best", "balanced", "fast"] as const).map((tier) => (
              <option key={tier} value={tier}>
                {tierLabel(tier)}
              </option>
            ))}
          </select>
        </label>
      )}
      {pendingImage && (
        <CropDialog
          title={t("cropImageTitle")}
          src={pendingImage}
          aspect={2 / 3}
          cropShape="rect"
          onConfirm={async (image) => setDraftImage(image)}
          onCancel={() => setPendingImage(null)}
        />
      )}
      {aiGenOpen && (
        <div className="modal-overlay" onClick={() => !aiGenerating && setAiGenOpen(false)}>
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("aiGenTitle")} onClick={(event) => event.stopPropagation()}>
            <h2>{t("aiGenTitle")}</h2>
            <label>{t("aiGenPromptLabel")}<textarea rows={3} value={aiPrompt} placeholder={t("aiGenPromptPlaceholder")} onChange={(event) => setAiPrompt(event.currentTarget.value)} /></label>
            <fieldset className="ai-gen-framing">
              <legend>{t("aiGenFramingLabel")}</legend>
              {(["full", "half"] as const).map((framing) => (
                <label key={framing}>
                  <input
                    type="radio"
                    name="ai-gen-framing"
                    checked={aiFraming === framing}
                    disabled={aiGenerating}
                    onChange={() => setAiFraming(framing)}
                  />
                  {framing === "full" ? t("aiGenFramingFull") : t("aiGenFramingHalf")}
                </label>
              ))}
            </fieldset>
            <label>{t("aiGenSourceLabel")}
              <div className="row">
                <select value={aiSource} onChange={(event) => setAiSource(event.currentTarget.value)} disabled={aiGenerating}>
                  {sourceOptions.map((source) => <option key={source} value={source}>{source === "api" ? t("aiGenSourceApi") : CLI_LABELS[source] ?? source}</option>)}
                  {!sourceOptions.includes(aiSource) && <option value={aiSource}>{CLI_LABELS[aiSource] ?? aiSource}</option>}
                </select>
                <button type="button" disabled={aiGenerating} onClick={onOpenAiSettings}>⚙ {t("aiTab")}</button>
              </div>
            </label>
            {sourceCannotGenerate && <div className="ai-gen-error" role="alert">{t("aiGenSourceNoImage", { provider: CLI_LABELS[aiSource] ?? aiSource })}</div>}
            {/* 生圖來源可以不經設定頁直接換，這裡也要講一次等一下的系統詢問是誰在問 */}
            {aiSource !== "api" && !sourceCannotGenerate && (
              <p className="cli-permission-note" role="note">
                {t("cliPermissionNote", { provider: CLI_LABELS[aiSource] ?? aiSource })}
              </p>
            )}
            {!sponsorUnlocked && <p role="note">{t("aiGenTrialNote", { n: Math.max(0, 3 - trialsUsed) })}</p>}
            {aiGenError && <div className="ai-gen-error" role="alert"><div>{t(explainAiError(aiGenError) ?? "aiGenFailed")}</div><small>{aiGenError}</small></div>}
            {galleryFiles.length > 0 && (
              <section aria-label={t("aiGalleryTitle")}>
                <h3>{t("aiGalleryTitle")}</h3>
                <div className="ai-gallery">
                  {galleryFiles.slice(0, galleryLoaded).map((file) => galleryImages[file] && (
                    <div className="ai-gallery-thumb" key={file}>
                      <button
                        type="button"
                        className="ai-gallery-pick"
                        title={t("aiGalleryPick")}
                        onClick={() => { setAiGenOpen(false); setPendingImage(galleryImages[file]); }}
                      >
                        <img src={galleryImages[file]} alt="" />
                      </button>
                      <button
                        type="button"
                        className="ai-gallery-delete"
                        aria-label={t("aiGalleryDeleteTitle")}
                        onClick={() => void deleteGalleryImage(file).catch((reason) => setAiGenError(String(reason)))}
                      >×</button>
                    </div>
                  ))}
                </div>
                {galleryFiles.length > galleryLoaded && <button type="button" onClick={() => void loadGalleryPage(galleryFiles, galleryLoaded)}>{t("aiGalleryLoadMore", { n: galleryFiles.length - galleryLoaded })}</button>}
              </section>
            )}
            {/* 主要動作放右下（2026-07-27 使用者拍板：此對話框例外，不置頂） */}
            <div className="ai-gen-footer">
              <button type="button" disabled={aiGenerating} onClick={() => setAiGenOpen(false)}>{t("cropCancel")}</button>
              <button type="button" className="ai-gen-submit" disabled={aiGenerating || sourceCannotGenerate} onClick={() => void generateImage()}>
                {aiGenerating ? t("aiGenerating") : `✨ ${t("aiGenBtn")}`}
              </button>
            </div>
          </div>
        </div>
      )}
      {aiGenLockedOpen && (
        <div className="modal-overlay" onClick={() => setAiGenLockedOpen(false)}>
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("aiGenLockedTitle")} onClick={(event) => event.stopPropagation()}>
            <div className="row"><button type="button" onClick={() => void openUrl(KOFI_URL)}>{t("sponsorBtn")}</button><button type="button" onClick={() => setAiGenLockedOpen(false)}>{t("closeBtn")}</button></div>
            <h2>{t("aiGenLockedTitle")}</h2><p>{t("aiGenLockedBody")}</p>
          </div>
        </div>
      )}
      {lightboxOpen && shownImage && (
        <div
          className="modal-overlay"
          role="dialog"
          aria-modal="true"
          aria-label={t("viewImageLabel")}
          onClick={() => setLightboxOpen(false)}
        >
          <img className="lightbox-image" src={shownImage} alt="" />
        </div>
      )}
      {croppingAvatar && shownImage && (
        <CropDialog
          title={t("cropAvatarTitle")}
          src={shownImage}
          aspect={1}
          cropShape="round"
          onConfirm={async (image) => setDraftAvatar(image)}
          onCancel={() => setCroppingAvatar(false)}
        />
      )}
    </form>
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
}: {
  world: string;
  worldName: string;
  scene: number;
  label: string;
  onBack: () => void;
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
  const activeCharacters = characters.filter((character) => !character.archived);
  const archivedCharacters = characters.filter((character) => character.archived);
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
  const [speaker, setSpeaker] = useState("");
  const [scene, setScene] = useState(0);
  const [sceneTitles, setSceneTitles] = useState<Record<string, string>>({});
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
  const [cardUiOpen, setCardUiOpen] = useState(false);
  // 這桌的匯入收據摘要：非空、且還沒開始跟 AI 對話，才顯示「復原上次匯入」按鈕
  const [importReceipts, setImportReceipts] = useState<ImportReceiptSummary[]>([]);
  const [chattedSinceImport, setChattedSinceImport] = useState(false);
  // 復原動作可能改動世界書／機制資料；世界設定畫面若剛好開著就靠改這把 key 強制整個重新掛載重載
  const [worldEditorRefreshKey, setWorldEditorRefreshKey] = useState(0);
  // 雙緩衝的兩格與目前露臉的那一格：新殼永遠先塞進背面那格，載好才翻面（見覆蓋層那段）
  const [shellSlots, setShellSlots] = useState<[string | null, string | null]>([null, null]);
  const [frontSlot, setFrontSlot] = useState(0);
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

  // 目前要顯示的卡片介面殼：由近到遠掃最近 10 則，跳過玩家發言（介面藏在 GM／角色那則的原文標籤裡）
  const cardInterfaceShell = useMemo(() => {
    const recent = events
      .slice(-10)
      .filter((event) => event.kind !== "player")
      .reverse()
      .map((event) => event.raw ?? event.text);
    // 空桌退回卡片自己的開場白：這類卡的開場就是一整頁選角畫面，玩家得先在那裡選了才有第一句話
    const openings = events.length === 0 ? cardInterfaces.map((card) => card.opening) : [];
    return findShell(cardInterfaces, [...recent, ...openings]);
  }, [events, cardInterfaces]);

  // 新殼一律先塞進背面那格去載；載好了才由 onLoad 翻面
  useEffect(() => {
    if (cardInterfaceShell === null) return;
    setShellSlots((slots) =>
      slots.includes(cardInterfaceShell)
        ? slots
        : frontSlot === 0
          ? [slots[0], cardInterfaceShell]
          : [cardInterfaceShell, slots[1]],
    );
  }, [cardInterfaceShell, frontSlot]);

  // 送出後最新一則是玩家發言、算不出新殼，但介面得留著等 GM 回話，所以看的是「現在有沒有殼可顯示」
  const cardShellReady = cardInterfaceShell !== null || shellSlots[frontSlot] !== null;

  const shellDocs = useMemo(
    () => shellSlots.map((shell) => (shell ? buildShellDocument(shell) : "")),
    [shellSlots],
  );

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
      if (data.kind !== "input") return;
      void submitTextRef.current(String(data.text ?? ""));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [cardUiOpen]);

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
    setTableState(state.state?.table ?? {});
    setTableTree(state.state?.tree ?? {});
    setTableJumps(state.state?.jumps ?? {});
    setBranchBindings(await loadBranchBindings(id));
    setEvents(transcript);
    setCharacters(cast);
    await loadCharacterImages(id, cast);
    await loadPlayerCard(id, state.player_card_id);
    setImportReceipts(
      await invoke<ImportReceiptSummary[]>("list_import_receipts", { worldId: id }).catch(() => []),
    );
    setChattedSinceImport(localStorage.getItem(chattedKey(id)) === "true");
    // 一個角色都沒有的桌（純世界書開局）對象預設 GM：不然送出去沒人接、輸入框也是鎖的
    setSpeaker(cast.find((character) => !character.archived)?.id ?? GM_TARGET);
    setEditingName(null);
    setEditingStateField(null);
    // 切桌就離開單幕閱讀／編輯畫面與前幕浮層，避免殘留上一桌的狀態
    setMainView(null);
    setActsOpen(false);
    setCardUiOpen(false);
    // 兩格緩衝一起清掉，否則上一桌的介面會被當成這桌的、按鈕出現在沒有介面的桌上
    setShellSlots([null, null]);
    setFrontSlot(0);
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
      return (
        <details
          className="state-tree-branch"
          key={key}
          open={depth === 0 || openBranchPaths.has(childPath.join("/"))}
        >
          <summary>
            {key}
            {characters.length > 0 && (
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
      setWorlds(await invoke<WorldMeta[]>("list_worlds"));
      // 世界設定畫面（世界書／機制帳本）若開著，資料在它自己的元件狀態裡，用 key 強制整個重掛載重載
      setWorldEditorRefreshKey((key) => key + 1);
      await refreshImportReceipts(table);
      await showMessage(
        t("undoLastImportDone") +
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
    setOpeningChoice(openings);
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
      setSpeaker(cast.find((character) => !character.archived)?.id ?? "");
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
  async function reorderCast(ordered: CharacterMeta[]) {
    setError("");
    const previous = characters;
    setCharacters([...ordered, ...characters.filter((character) => character.archived)]);
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
    const { text, raw, next, state_updates } = await invoke<{
      text: string;
      raw: string | null;
      next: string | null;
      // 後端還沒上線這欄時是 undefined，當空陣列處理，別讓面板炸掉
      state_updates?: { path: string; value: string }[];
    }>("gm_narrate", {
      worldId: table,
      onDelta,
    });
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
  // 一有新內容就作廢（同復原疊的道理：位置已被後話蓋掉，退回會連新句子一起丟）
  const canUndoScene = scene > 0 && events.length === 1 && generating === null;

  // 發言對象可能是 GM（沒有角色卡），顯示名與顏色在這裡收斂一次
  const gmTargeted = speaker === GM_TARGET;
  const targetName = gmTargeted ? "GM" : (metaOf(speaker)?.name ?? speaker);
  const requestReplyLabel = t("requestReplyBtn", {
    name: speaker ? targetName : t("characterFallback"),
  });

  // 幕的顯示標籤：有取到幕名就「第 n 幕：幕名」，沒有就沿用「第 n 幕」；n 從 1 起算，內部場號 0 起算
  const sceneDisplayLabel = (n: number) => {
    const title = sceneTitles[String(n)];
    return title ? t("sceneWithTitle", { n: n + 1, title }) : t("sceneLabel", { n: n + 1 });
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
                <img className="gm-book" src={gmBook} alt="" />
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
                {archivedCharacters.map((character) => (
                  <div className="archive-row" key={character.id}>
                    <span>{character.name}</span>
                    {/* 隱藏卡也要進得了編輯器：轉成世界書條目只能在隱藏狀態下按 */}
                    <button type="button" onClick={() => void editCard(character.id)}>
                      {t("editBtn")}
                    </button>
                    <button type="button" onClick={() => void restoreCharacter(character.id)}>
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
                ))}
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
            {/* 沒有可用殼的桌完全不出現這顆鈕——不是每張卡都帶介面 */}
            {cardShellReady && (
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

        {(hasStateBar || Object.keys(tableTree).length > 0) && (
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
              onBack={() => setMainView(null)}
              leaveGuard={leaveGuard}
              convertColor={PALETTE[characters.length % PALETTE.length]}
              hasPlayerCard={playerCard !== null}
              onEntryConverted={async (asPlayer) => {
                await refreshCharacters();
                if (asPlayer) {
                  const state = await invoke<WorldState>("read_state", { worldId: table });
                  await loadPlayerCard(table, state.player_card_id);
                }
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
                      <img className="opt-avatar" src={gmBook} alt="" />
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

      {/* 卡片自帶介面整面取代對話；殼本身已含敘事畫面，不用再疊聊天記錄 */}
      {cardUiOpen && cardShellReady && (
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
          <button
            type="button"
            className="modal-close card-interface-close"
            aria-label={t("cardInterfaceClose")}
            onClick={() => setCardUiOpen(false)}
          >
            ✕
          </button>
          {/* 雙緩衝：新畫面先在背面那格載好才翻上來，在那之前舊畫面一直在，中途不會空白
              （ST 是每則訊息各自一個 iframe、舊的留著，這是單格版本的等效作法）。
              key 固定成格號：換 key 會重新掛載，等於白載一次。 */}
          {([0, 1] as const).map((slot) => (
            <iframe
              key={slot}
              className={`card-interface-frame${slot === frontSlot ? "" : " card-interface-preload"}`}
              sandbox="allow-scripts"
              srcDoc={shellDocs[slot]}
              title={t("cardInterfaceOpen")}
              onLoad={() => {
                if (slot !== frontSlot && shellSlots[slot] === cardInterfaceShell) setFrontSlot(slot);
              }}
            />
          ))}
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
            <p>{t("openingLineAsk")}</p>
            <div className="opening-choice-list">
              {openingChoice.map((opening, index) => {
                // 點列只展開全文，貼出的鈕在框外底部——開場白動輒上千字，按鈕若跟在全文後面
                // 得整段捲到底才按得到，而滿是標記的開場白根本沒必要逐字看完
                const expanded = openingExpanded === index;
                return (
                  <div className="opening-choice-item" key={index}>
                    <button
                      type="button"
                      className="opening-choice-head"
                      aria-expanded={expanded}
                      onClick={() => setOpeningExpanded(expanded ? null : index)}
                    >
                      <strong>{t("openingChoiceItem", { n: index + 1 })}</strong>
                      <span>{expanded ? "" : openingPreview(opening)}</span>
                    </button>
                    {expanded && <div className="opening-choice-full">{opening}</div>}
                  </div>
                );
              })}
            </div>
            <div className="ai-gen-footer">
              {openingExpanded !== null && openingChoice[openingExpanded] !== undefined && (
                <button
                  type="button"
                  className="footer-lead"
                  onClick={() => void postOpening(openingChoice[openingExpanded])}
                >
                  {t("openingLineOk")}
                </button>
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
