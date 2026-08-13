import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm, message as showMessage, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { detectLang, Lang, normalizeLang, setLang, t } from "./i18n";
import { decideImportRoute } from "./import-routing";
import { isCharacterHidden } from "./character-visibility";
import { tierLabel } from "./model-catalog";
import { prefetchModelCatalogs } from "./model-catalog-store";
import { resolveTheme, TEXT_SIZE_DEFAULT, TEXT_SIZE_PX } from "./appearance";
import { AppConfig, SceneLabel, StateNode, TranscriptEvent, WorldbookEntry, WorldState } from "./backend-contracts";
import { CharacterMeta, PALETTE } from "./card-model";
import { cliConnectedKey } from "./cli";
import { useDragReorder } from "./drag-reorder";
import { useCardInterfaceController } from "./controllers/useCardInterfaceController";
import { useCharacterController } from "./controllers/useCharacterController";
import { useChatController } from "./controllers/useChatController";
import { loadBranchBindings, treeValueAt, useTableStateController } from "./controllers/useTableStateController";
import { ActReader, EditPane, ErrorNote, StoryText } from "./views/atoms";
import { CardEditor } from "./views/CardEditor";
import { SettingsWindow } from "./views/SettingsWindow";
import { WorldEditor } from "./views/WorldEditor";
import gmBook from "./assets/gm-book.png";
import "./App.css";

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

// 串流中的旁白尾端會冒出狀態區塊，整則寫完才由後端剝乾淨；
// 這裡先切掉，免得玩家每回合都看到一段圍欄或標籤閃過去
function narrationStreamText(text: string) {
  const marker = text.search(/```|<details|<status|<UpdateVariable/i);
  return marker === -1 ? text : text.slice(0, marker);
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

// 值裡的字面 {{user}} 只在顯示時換成玩家名（模型上下文與存檔仍是原文，後端注入前才代換）；
// 大小寫不分、容許中間空白（{{ user }}），其他巨集不動
const USER_MACRO = /\{\{\s*user\s*\}\}/gi;
function displayUserMacro(value: string, playerName: string): string {
  return value.replace(USER_MACRO, playerName);
}

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

const CLI_IDS = ["claude", "codex", "agy", "grok"] as const;

// 換場提醒門檻：粗略以字元數估算紀錄長度，不精算 token。
// 快取上線後換幕不再省額度（摘要與換幕後首輪都全額計價，約等於連跑四輪），
// 提醒的理由改成「紀錄長到模型顧不上前面」，門檻從 8000 提到 30000（2026-08-04 實測拍板）。
const SCENE_LENGTH_HINT_CHARS = 30000;

// 離開太久的換幕提醒還要紀錄夠長才有意義：短紀錄重建本來就便宜，換幕反而多花一次摘要錢。
// 保溫仍照樣停在三次（那是省錢邏輯），這個門檻只決定要不要出聲提醒。
const SCENE_AWAY_HINT_MIN_CHARS = 8000;

function openingPreview(text: string) {
  const preview = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .slice(0, 2)
    .join(" ");
  return preview.length > 120 ? `${preview.slice(0, 119)}…` : preview;
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

function App() {
  const [worlds, setWorlds] = useState<WorldMeta[]>([]);
  // table 存桌 id；顯示名一律經 tableName（見下）從 worlds 查
  const [table, setTable] = useState("");
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [sponsorUnlocked, setSponsorUnlocked] = useState(false);
  // 角色卡編輯器每次 render 掛上「可以離開嗎」；側欄任何會換掉編輯畫面的入口都先問它
  const leaveGuard = useRef<(() => Promise<boolean>) | null>(null);
  const [speaker, setSpeaker] = useState("");
  const [scene, setScene] = useState(0);
  const [sceneTitles, setSceneTitles] = useState<Record<string, string>>({});
  const [sceneLabels, setSceneLabels] = useState<Record<string, SceneLabel>>({});
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
  // 這桌的匯入收據摘要：非空、且還沒開始跟 AI 對話，才顯示「復原上次匯入」按鈕
  const [importReceipts, setImportReceipts] = useState<ImportReceiptSummary[]>([]);
  const [chattedSinceImport, setChattedSinceImport] = useState(false);
  // 復原動作可能改動世界書／機制資料；世界設定畫面若剛好開著就靠改這把 key 強制整個重新掛載重載
  const [worldEditorRefreshKey, setWorldEditorRefreshKey] = useState(0);
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

  // 狀態列／狀態樹：平欄、樹、跳動記號、分支指認與編輯中的那一格都在 controller 裡。
  // 掛在 error 之後：注入的 onError 就是 setError（useState 的 setter，identity 穩定）
  const tableState = useTableStateController({ worldId: table, onError: setError });

  // 角色名單、本幕出場集合、玩家卡與角色圖／GM 圖三份快取都在 controller 裡。
  // 發言對象留在 App（聊天域也要用），角色被刪時由 characters.noteRemoved 回報該撥給誰。
  const characters = useCharacterController({ worldId: table, onError: setError });
  const castDrag = useDragReorder(
    characters.active,
    (character) => character.id,
    (ordered) => void characters.reorder(ordered),
  );

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
  const markCliConnectedFromChat = useCallback(async () => {
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
  }, []);

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

  // AI 回一輪後桌次依最後活動重排，聊天流程收尾要重讀清單
  const refreshWorlds = useCallback(async () => {
    setWorlds(await invoke<WorldMeta[]>("list_worlds"));
  }, []);

  // 向 AI 發出對話請求：從這一刻起收掉「復原上次匯入」，免得演到一半誤按整張卡沒了
  const noteChatRequest = useCallback(() => {
    if (!table) return;
    localStorage.setItem(chattedKey(table), "true");
    setChattedSinceImport(true);
  }, [table]);

  const closeOpeningChoice = useCallback(() => setOpeningChoice(null), []);

  // 發言對象可能是 GM（沒有角色卡）：送出走旁白那條，晶片的名字與顏色也另一套
  const gmTargeted = speaker === GM_TARGET;

  // 逐字稿、收回堆疊、生成中狀態、輸入框與整條對話流程都在 controller 裡。
  // 掛在 cardInterface 之前：那支要吃這裡的 submitText。
  const chat = useChatController({
    worldId: table,
    scene,
    config,
    speaker,
    gmTargeted,
    metaOf: characters.metaOf,
    playerName: characters.player?.name,
    castCount: characters.active.length,
    onArrived: characters.onArrived,
    refreshState: tableState.refresh,
    refreshWorlds,
    noteChatStarted: noteChatRequest,
    markCliConnected: markCliConnectedFromChat,
    closeOpeningChoice,
    onError: setError,
  });

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [chat.events, chat.generating, chat.streamText]);

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
  }, [table, mainView, characters.list]);

  // 卡片介面：介面腳本／重構殼／覆蓋層開關與沙盒訊息都在 controller 裡，
  // 這裡只餵它需要的四樣（送出函式會隨對話狀態換新，controller 內用 latest-ref 收）
  const cardInterface = useCardInterfaceController({
    worldId: table,
    events: chat.events,
    tableTree: tableState.tree,
    submitText: chat.submitText,
  });

  async function enterTable(id: string, loaded: AppConfig) {
    const state = await invoke<WorldState>("read_state", { worldId: id });
    const transcript = await invoke<TranscriptEvent[]>("read_transcript", {
      worldId: id,
      scene: state.current_scene,
    });
    const cast = await invoke<CharacterMeta[]>("list_characters", { worldId: id });
    // 綁定清單先讀完再進同步區：hydrate 只做 state commit，中間不留 await（免得 React batch
    // 被切斷，畫面出現「新桌的狀態樹＋舊桌的訊息」這種跨桌混合）
    const bindings = await loadBranchBindings(id);
    // 本幕已出場集合：auto_hidden 卡是否落在主區靠這份初始化，讀不到就當空集合（全部從隱藏區起算）
    const appearances = await invoke<{ character_ids: string[]; person_titles: string[] }>(
      "scene_appearances",
      { worldId: id },
    ).catch(() => ({ character_ids: [], person_titles: [] }));
    const appearanceIds = new Set(appearances.character_ids);
    const receipts = await invoke<ImportReceiptSummary[]>("list_import_receipts", { worldId: id }).catch(() => []);
    setTable(id);
    setScene(state.current_scene);
    setSceneTitles(state.scene_titles ?? {});
    setSceneLabels(state.scene_labels ?? {});
    tableState.hydrate(state.state, bindings);
    chat.hydrate(transcript);
    // 角色圖／GM 圖／玩家卡由 controller 自己的 effect 補：hydrate 先把上一桌的清掉，
    // 這裡一路同步提交，不讓 await 把 React batch 切成跨桌混合的中間畫面
    characters.hydrate(cast, appearanceIds, state.player_card_id);
    setImportReceipts(receipts);
    setChattedSinceImport(localStorage.getItem(chattedKey(id)) === "true");
    // 一個角色都沒有的桌（純世界書開局）對象預設 GM：不然送出去沒人接、輸入框也是鎖的；
    // 隱藏區的卡（含本幕還沒出場的 auto_hidden）不當預設對象，跟側欄主區顯示一致
    setSpeaker(cast.find((character) => !isCharacterHidden(character, appearanceIds))?.id ?? GM_TARGET);
    setEditingName(null);
    tableState.clearEdit();
    // 切桌就離開單幕閱讀／編輯畫面與前幕浮層，避免殘留上一桌的狀態
    setMainView(null);
    setActsOpen(false);
    cardInterface.close();
    if (loaded.preferences["last_world"] !== id) {
      const next = { ...loaded, preferences: { ...loaded.preferences, last_world: id } };
      await invoke("write_config", { config: next });
      setConfig(next);
    }
  }

  async function switchTable(id: string) {
    if (!config || id === table || chat.busy) return;
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
    if (!config || chat.busy) return;
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
    if (!config || chat.busy) return;
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

  // 表單交給瀏覽器處理 Enter，中文輸入法選字時不會提前送出。
  function stateFieldForm(path: string[], tree: boolean, label: string) {
    const value = tableState.editing?.value ?? "";
    return (
      <form
        className="state-bar-field-form"
        onSubmit={(event) => {
          event.preventDefault();
          void tableState.save(path, tree, value);
        }}
      >
        <input
          className="state-bar-input"
          autoFocus
          value={value}
          aria-label={label}
          onChange={(event) => {
            const next = event.currentTarget.value;
            tableState.changeEditValue(next);
          }}
          onBlur={() => void tableState.save(path, tree, value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              tableState.cancelEdit();
            }
          }}
        />
      </form>
    );
  }

  // 一列點著就能改的欄位：平欄與樹葉子共用，差別只在存回哪裡
  function stateLeafRow(path: string[], tree: boolean, label: string) {
    const editing =
      tableState.editing?.tree === tree &&
      tableState.editing.path.length === path.length &&
      tableState.editing.path.every((segment, index) => segment === path[index]);
    const value = tree ? treeValueAt(tableState.tree, path) : (tableState.fields[path[0]] ?? "");
    const jumpMark = tableState.jumps[path.join(".")];
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
              onClick={() => tableState.beginEdit(path, tree, value)}
            >
              {value ? displayUserMacro(value, characters.player?.name || t("playerLabel")) : t("stateEmptyValue")}
            </button>
            {jumpMark && (
              <button
                className="state-bar-jump"
                type="button"
                title={t("stateJumpHint")}
                onClick={() => void tableState.markCounter(path)}
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
    const player = characters.player;
    const bound = player && tableState.bindings.find((b) => b.characterId === player.id);
    const set = new Set<string>();
    if (bound) {
      for (let depth = 1; depth <= bound.path.length; depth += 1) {
        set.add(bound.path.slice(0, depth).join("/"));
      }
    }
    return set;
  }, [characters.player, tableState.bindings]);

  // 樹狀折疊：分支一層層收起來，預設展開第一層與玩家自己那支；summary 上附分支指認下拉
  function stateTreeNodes(nodes: Record<string, StateNode>, path: string[], depth: number) {
    return Object.entries(nodes).map(([key, node]) => {
      const childPath = [...path, key];
      if (typeof node === "string") return stateLeafRow(childPath, true, key);
      const bound = tableState.bindings.find(
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
            {characters.list.length > 0 && !isList && (
              <select
                className="state-tree-bind"
                aria-label={t("stateBranchBindAria")}
                title={t("stateBranchBindHint")}
                value={bound?.characterId ?? ""}
                onClick={(event) => event.stopPropagation()}
                onPointerDown={(event) => event.stopPropagation()}
                onChange={(event) => {
                  const nextId = event.currentTarget.value;
                  if (nextId) void tableState.bind(nextId, childPath);
                  else if (bound) void tableState.bind(bound.characterId, null);
                }}
              >
                <option value="">{t("stateBranchUnbound")}</option>
                {characters.active.map((character) => (
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

  // 換場：把目前場景公開紀錄壓成一則前情提要，寫進新場景開頭，current_scene +1
  async function advanceScene() {
    setError("");
    chat.beginNarration();
    try {
      await invoke<number>("advance_scene", { worldId: table });
      await enterTable(table, config!);
      chat.noteTurnDone();
    } catch (reason) {
      setError(String(reason));
    } finally {
      chat.endNarration();
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
    chat.beginNarration();
    try {
      await invoke("regenerate_scene_summary", { worldId: table });
      await enterTable(table, config!);
      chat.noteTurnDone();
    } catch (reason) {
      setError(String(reason));
    } finally {
      chat.endNarration();
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

  async function refreshImportReceipts(worldId: string) {
    setImportReceipts(
      await invoke<ImportReceiptSummary[]>("list_import_receipts", { worldId }).catch(() => []),
    );
    // 剛匯入（或剛復原一筆）＝又回到「還沒開演」的狀態，按鈕重新給
    localStorage.removeItem(chattedKey(worldId));
    setChattedSinceImport(false);
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
      const cast = await characters.refresh();
      // 發言對象指向的角色被這次復原刪掉了（不管是不是巧合）就改回 GM，不然輸入框對著空氣
      if (speaker && speaker !== GM_TARGET && !cast.some((character) => character.id === speaker)) {
        setSpeaker(GM_TARGET);
      }
      await cardInterface.refreshInterfaces(table);
      // 復原的若是重構套用，磁碟上的介面殼檔已被刪，前端快取跟著重問一次
      await cardInterface.refreshShell(table);
      // 復原的若是 PNG 世界書匯入，GM 卡的圖也被刪了，重讀一次回到書本圖
      await characters.reloadGmImage();
      // 貼出的開場白被一起收掉：檯面與狀態快照都變了，重讀這一幕
      if (report.removed_opening) {
        await chat.reload();
        await tableState.refresh();
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
    if (!config || chat.busy) return;
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
      color: PALETTE[characters.list.length % PALETTE.length],
    });
    await characters.refresh(worldId);
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
    await characters.reloadGmImage(worldId);
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
    const interfaces = await cardInterface.refreshInterfaces(worldId);
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
    cardInterface.openIfDrawable(interfaces);
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
    if (translated !== null) await chat.postOpening(translated);
  }

  // 建卡或改名存檔後：名單與圖片重載；id 全程不變，只有「新卡剛存下」要轉正畫面並選為發言對象
  async function finishCardSaved(id: string) {
    const wasNew = mainView?.kind === "new-character";
    await characters.refresh();
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
    await characters.reloadPlayer(id);
  }

  // 角色被隱藏或刪除後的善後：名單重載（controller）、發言對象改人、關掉編輯面板
  async function finishRemoval(id: string) {
    const nextSpeaker = await characters.noteRemoved(id, speaker);
    if (nextSpeaker !== null) setSpeaker(nextSpeaker);
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
    if (characters.player) {
      setMainView({ kind: "player", id: characters.player.id });
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

  // 隱藏區與角色卡編輯畫面共用同一條刪除路徑：確認框與刪檔在 controller，這裡接善後
  async function deleteCharacter(id: string) {
    try {
      if (await characters.remove(id)) await finishRemoval(id);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function deletePlayerCard(id: string) {
    if (await characters.removePlayer(id)) setMainView(null);
  }

  // 剛換完幕、這一幕只有那則前情提要＝還沒開始玩，兩條補救路都還來得及。
  // 一有新內容就作廢（同復原疊的道理：位置已被後話蓋掉，退回會連新句子一起丟）。
  // 分岔來的幕排除掉：那一則是複製來的真實對話，重寫會直接把它蓋成摘要
  const canUndoScene =
    scene > 0 &&
    chat.events.length === 1 &&
    !chat.busy &&
    !sceneLabels[String(scene)]?.forked;

  const targetName = gmTargeted ? "GM" : (characters.metaOf(speaker)?.name ?? speaker);
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
  const generatingMeta = chat.generating !== null ? characters.metaOf(chat.generating.id) : undefined;

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
    ...Object.keys(tableState.fields)
      .filter((key) => !["time", "place", "present"].includes(key))
      .map((key) => ({ key, label: key })),
  ];
  const stateValue = (key: string) => tableState.fields[key] || t("stateEmptyValue");

  // 換場提醒：粗估目前場景累計字元數，超過門檻就在送出鈕旁小字提醒（不擋操作）
  const sceneChars = chat.events.reduce((sum, event) => sum + event.text.length, 0);
  const sceneTooLong = sceneChars > SCENE_LENGTH_HINT_CHARS;
  // 離開太久＋紀錄夠長才提醒換幕：兩者缺一，換幕都是白花一次摘要錢
  const showAwayHint = chat.awayTooLong && sceneChars > SCENE_AWAY_HINT_MIN_CHARS;

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
            <button className="new-table" onClick={newTable} disabled={chat.busy}>
              {t("newTable")}
            </button>
            <button className="gen-table" onClick={() => setGenTableOpen(true)} disabled={chat.busy}>
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
                    disabled={chat.busy}
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
                {characters.gmImage ? (
                  <img className="tcard-image" src={characters.gmImage} alt="" />
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
              className={`tcard tcard-player${characters.player ? "" : " tcard-player-empty"}`}
              title={t(characters.player ? "playerCardHint" : "playerCardEmptyHint")}
              onClick={() => void openPlayerCard()}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  void openPlayerCard();
                }
              }}
            >
              {characters.player ? (
                <>
                  <span className="tcard-art">
                    {characters.player.show_image && characters.playerImage ? (
                      <img className="tcard-image" src={characters.playerImage} alt="" />
                    ) : characters.playerAvatar ? (
                      <img className="avatar-round tcard-avatar" src={characters.playerAvatar} alt="" />
                    ) : (
                      <span aria-hidden="true">{characters.player.avatar}</span>
                    )}
                  </span>
                  <span className="tcard-body">
                    <span className="tcard-name-row">
                      <span className="tcard-plate">{characters.player.name}</span>
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
                  {c.show_image && characters.images[c.id] ? (
                    <img className="tcard-image" src={characters.images[c.id]} alt="" />
                  ) : characters.avatars[c.id] ? (
                    <img className="avatar-round tcard-avatar" src={characters.avatars[c.id]} alt="" />
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
          {characters.archived.length > 0 && (
            <details className="archive-section">
              <summary>{t("archiveSectionTitle")}</summary>
              <div className="archive-list">
                {characters.archived.map((character) => {
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
                          void (isAutoHidden ? characters.restoreAutoHidden(character.id) : characters.restore(character.id))
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
            {mainView === null && cardInterface.shellReady && (
              <button type="button" onClick={() => cardInterface.open()}>
                {t("cardInterfaceOpen")}
              </button>
            )}
            <button
              type="button"
              title={t("sceneAdvanceHint")}
              aria-label={t("sceneAdvance")}
              disabled={chat.busy || chat.events.length === 0}
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

        {mainView === null && (hasStateBar || Object.keys(tableState.tree).length > 0) && (
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
            {stateTreeNodes(tableState.tree, [], 0)}
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
                    : t("editCardSummary", { name: characters.metaOf(cardView.id)?.name ?? "" })
            }
          >
            <CardEditor
              world={table}
              characterId={cardView.id}
              isNew={cardView.kind === "new-character" || cardView.kind === "new-player"}
              isPlayer={editingPlayerCard}
              newCardColor={PALETTE[characters.list.length % PALETTE.length]}
              imageDataUrl={
                editingPlayerCard ? characters.playerImage ?? undefined : characters.images[cardView.id]
              }
              avatarImgUrl={
                editingPlayerCard ? characters.playerAvatar ?? undefined : characters.avatars[cardView.id]
              }
              onImagesChanged={() =>
                editingPlayerCard
                  ? characters.reloadPlayer(characters.player?.id ?? null)
                  : characters.reloadImages()
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
              convertColor={PALETTE[characters.list.length % PALETTE.length]}
              onEntryConverted={async () => {
                await characters.refresh();
              }}
              onRefactorApplied={async () => {
                await characters.refresh();
                // 重構把原卡拆成一群 NPC：發言對象一律撥回 GM，
                // 不然玩家一開口變成在跟其中一名拆出來的角色對話，回覆完全對不上
                setSpeaker(GM_TARGET);
                // 合併升格可能把某位角色指定為玩家卡（要點 4），跟單條「轉成角色卡」的
                // asPlayer 分支一樣重讀一次，讓側欄玩家卡即時反映。
                const state = await invoke<WorldState>("read_state", { worldId: table });
                await characters.reloadPlayer(state.player_card_id);
                await cardInterface.refreshInterfaces(table);
                await cardInterface.refreshShell(table);
                await tableState.refresh();
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
              {chat.events.map((event, index) => {
                if (event.kind === "dialogue" || event.kind === "player") {
                  const meta = characters.metaOf(event.speaker_id);
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
              {chat.generating !== null && chat.generating.kind === "dialogue" && (
                <div
                  className="message message-dialogue"
                  style={{ ["--fac" as string]: generatingMeta?.color ?? "#888888" }}
                >
                  <div className="pb-name">
                    <span className="pb-plate">{generatingMeta?.name ?? ""}</span>
                  </div>
                  {chat.streamText ? (
                    <span className="text">{chat.streamText}</span>
                  ) : (
                    <span className="typing" aria-label={t("typing", { name: generatingMeta?.name ?? "" })}>
                      <i />
                      <i />
                      <i />
                    </span>
                  )}
                </div>
              )}
              {chat.generating !== null && chat.generating.kind === "narration" && (
                <div className="message message-narration">
                  {narrationStreamText(chat.streamText) ? (
                    <span className="text">{narrationStreamText(chat.streamText)}</span>
                  ) : (
                    <span className="typing" aria-label={t("typing", { name: "GM" })}>
                      <i />
                      <i />
                      <i />
                    </span>
                  )}
                </div>
              )}
              {chat.canRestore && !chat.busy && (
                <div className="undo-restore">
                  <button type="button" onClick={() => void chat.restoreUndone()}>
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
            <form className="composer" onSubmit={chat.send}>
              {speaker && (
                <div className="composer-opts">
                  <span
                    className="opt-target"
                    title={gmTargeted ? t("gmTargetHint") : t("castHint", { name: targetName })}
                    style={{
                      ["--fac" as string]: gmTargeted ? GM_COLOR : (characters.metaOf(speaker)?.color ?? "#888888"),
                    }}
                  >
                    {gmTargeted ? (
                      characters.gmImage ? (
                        <img className="avatar-round opt-avatar gm-opt-avatar" src={characters.gmImage} alt="" />
                      ) : (
                        <img className="opt-avatar" src={gmBook} alt="" />
                      )
                    ) : characters.avatars[speaker] ? (
                      <img className="avatar-round opt-avatar" src={characters.avatars[speaker]} alt="" />
                    ) : (
                      <span aria-hidden="true">{characters.metaOf(speaker)?.avatar ?? "🎭"}</span>
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
                value={chat.input}
                onChange={(e) => chat.setInput(e.currentTarget.value)}
                placeholder={
                  speaker
                    ? t("composerPlaceholder", { name: targetName })
                    : characters.active.length === 0
                      ? t("composerNoCharacter")
                      : t("composerNoTarget")
                }
                disabled={(!speaker && characters.active.length === 0) || chat.busy}
              />
              {/* 送出擺最左：它跟輸入框是同一件事，右邊那三顆是交給 AI 的動作
                  （2026-07-28 使用者回報：送出在右下容易誤按成「請某某發言」） */}
              <div className="composer-send">
                <div className="composer-primary-action">
                  <button
                    type="submit"
                    disabled={(!speaker && characters.active.length === 0) || chat.busy}
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
                    onClick={() => void chat.undoLast()}
                    disabled={chat.busy || chat.events.length === 0}
                    title={t("undoLastHint")}
                  >
                    ↩ {t("undoLast")}
                  </button>
                  <button
                    className="request-reply"
                    type="button"
                    onClick={() => void chat.replyFromTarget()}
                    disabled={!speaker || chat.busy}
                    title={`${requestReplyLabel} — ${t("requestReplyHint")}`}
                    aria-label={requestReplyLabel}
                  >
                    <span className="request-reply-label">{requestReplyLabel}</span>
                  </button>
                  <button
                    type="button"
                    onClick={chat.gmNarrate}
                    disabled={chat.busy}
                    title={t("gmNarrateHint")}
                  >
                    {t("gmNarrate")}
                  </button>
                  <button
                    type="button"
                    onClick={chat.gmAdvance}
                    disabled={chat.busy || characters.active.length === 0}
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
      {mainView === null && cardInterface.uiOpen && cardInterface.shellReady && (
        <div className="card-interface-overlay">
          {chat.generating !== null && (
            <div className="card-interface-status" role="status">
              {t("typing", { name: chat.generating.kind === "narration" ? "GM" : (generatingMeta?.name ?? "GM") })}
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
              onClick={() => cardInterface.close()}
            >
              ✕
            </button>
          </div>
          {/* 單 iframe 直繪：key＝殼指紋，殼一換整支重掛（掛載時 srcdoc 就在，必然載入）。
              殼更新瞬間可能閃一下白，換來顯示的確定性。 */}
          {cardInterface.shellDoc !== null && (
            <iframe
              key={cardInterface.shellKey}
              className="card-interface-frame"
              sandbox="allow-scripts"
              srcDoc={cardInterface.shellDoc}
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
                disabled={chat.busy}
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
                    onClick={() => void chat.postOpening(openingChoice[openingExpanded])}
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
