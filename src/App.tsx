import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm, message as showMessage, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { detectLang, Lang, normalizeLang, setLang, t } from "./i18n";
import { isCharacterHidden } from "./character-visibility";
import { prefetchModelCatalogs } from "./model-catalog-store";
import { resolveTheme, TEXT_SIZE_DEFAULT, TEXT_SIZE_PX } from "./appearance";
import {
  AppConfig,
  SceneLabel,
  TranscriptEvent,
  WorldMeta,
  WorldState,
} from "./backend-contracts";
import { CharacterMeta, PALETTE } from "./card-model";
import { cliConnectedKey } from "./cli";
import { useCardInterfaceController } from "./controllers/useCardInterfaceController";
import { useCharacterController } from "./controllers/useCharacterController";
import { useChatController } from "./controllers/useChatController";
import { useImportController } from "./controllers/useImportController";
import { loadBranchBindings, useTableStateController } from "./controllers/useTableStateController";
import { ErrorNote } from "./views/atoms";
import { CardInterfaceOverlay } from "./views/CardInterfaceOverlay";
import { GenerateTableDialog } from "./views/GenerateTableDialog";
import { ImportDialogs } from "./views/ImportDialogs";
import { MainView } from "./views/MainView";
import { Onboarding } from "./views/Onboarding";
import { PlayView } from "./views/PlayView";
import { SettingsWindow } from "./views/SettingsWindow";
import { TableSidebar } from "./views/TableSidebar";
import { StateBar, WorkspaceHeader } from "./views/WorkspaceHeader";
import "./App.css";

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

// 發言對象是 GM 時 speaker 存這個代號（純前端狀態，不會寫進紀錄）；GM 以旁白回應
const GM_TARGET = "__GM__";
// GM 卡的銅金色：發言對象晶片沿用書皮的 --fac，與角色卡的陣營色區隔
const GM_COLOR = "#8a6a3c";

// 這桌向 AI 發過對話請求了沒（每桌一把）。開演之後復原＝把演到一半的角色卡連同後續編輯一起刪掉，
// 所以按鈕要收起來；記在瀏覽器端，重開 app 也不該讓它又冒出來讓人誤按。
const chattedKey = (worldId: string) => `chatted_since_import:${worldId}`;
const CLI_IDS = ["claude", "codex", "agy", "grok"] as const;

// 認證失敗的下一步依傳輸而異：API 是換金鑰、CLI 是重新登入。
// 設定還沒載入就回 undefined——猜錯會把人指去錯的地方，中性文案還比較誠實。
const transportOf = (config: AppConfig | null) =>
  config ? String(config.preferences["transport"] ?? "api") : undefined;

function App() {
  const [worlds, setWorlds] = useState<WorldMeta[]>([]);
  // table 存桌 id；顯示名一律經 tableName（見下）從 worlds 查
  const [table, setTable] = useState("");
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [sponsorUnlocked, setSponsorUnlocked] = useState(false);
  // 角色卡編輯器每次 render 掛上「可以離開嗎」；任何會換掉編輯畫面的入口都先問它
  const leaveGuard = useRef<(() => Promise<boolean>) | null>(null);
  // 守門有入口落在 controller 的 callback 裡（匯入路由框的「開新桌並匯入」），那些閉包不隨
  // 主欄畫面重建，走 ref 才問得到當下這個畫面（比照 chatConfigRef）
  const canLeaveRef = useRef<() => Promise<boolean>>(async () => true);
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
  const [error, setError] = useState("");
  // 狀態列只給有匯入狀態列規則的桌：其他桌整條不掛上去，也就打不開
  const [hasStateBar, setHasStateBar] = useState(false);
  // 這桌向 AI 開演了沒：聊天域寫、匯入域清、側欄的「復原上次匯入」讀，留在 App 當共用旗標
  const [chattedSinceImport, setChattedSinceImport] = useState(false);
  // 復原動作可能改動世界書／機制資料；世界設定畫面若剛好開著就靠改這把 key 強制整個重新掛載重載
  const [worldEditorRefreshKey, setWorldEditorRefreshKey] = useState(0);
  // 生成對話框只留開關在 App：草稿與三支生成流程都在 GenerateTableDialog 自己身上
  const [genTableOpen, setGenTableOpen] = useState(false);

  // 狀態列／狀態樹：平欄、樹、跳動記號、分支指認與編輯中的那一格都在 controller 裡。
  // 掛在 error 之後：注入的 onError 就是 setError（useState 的 setter，identity 穩定）
  const tableState = useTableStateController({ worldId: table, onError: setError });

  // 角色名單、本幕出場集合、玩家卡與角色圖／GM 圖三份快取都在 controller 裡。
  // 發言對象留在 App（聊天域也要用），角色被刪時由 characters.noteRemoved 回報該撥給誰。
  const characters = useCharacterController({ worldId: table, onError: setError });

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
    onError: setError,
  });

  // 切桌、匯入卡、改完世界書都要重問一次這桌有沒有狀態列。
  // 狀態樹與分支指認一起重讀：匯入卡才建好的樹、建卡才比對上的同名分支，
  // 都只在這幾個時機變動，不重讀的話畫面要切走再切回來才看得到
  useEffect(() => {
    if (!table) return;
    let stale = false;
    invoke<boolean>("world_has_state_bar", { worldId: table })
      .then((has) => {
        if (!stale) setHasStateBar(has);
      })
      .catch(() => {});
    void tableState.refresh();
    return () => {
      stale = true;
    };
  }, [table, mainView, characters.list, tableState.refresh]);

  // 卡片介面：介面腳本／重構殼／覆蓋層開關與沙盒訊息都在 controller 裡，
  // 這裡只餵它需要的四樣（送出函式會隨對話狀態換新，controller 內用 latest-ref 收）
  const cardInterface = useCardInterfaceController({
    worldId: table,
    events: chat.events,
    tableTree: tableState.tree,
    submitText: chat.submitText,
  });

  // 一桌一卡：匯入成功後，還掛自動名的桌直接改成卡名；自訂過名字的桌不動
  // （匯入 controller 要注入這支，所以擺在 hook 之前；hasAutoName 是宣告式函式，往下找得到）
  const adoptImportName = useCallback(
    async (name: string | null | undefined) => {
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
    },
    [worlds, table],
  );

  // 匯完把對話目標指過去；null＝指到 GM（GM_TARGET 與 speaker 都留在 App）
  const focusSpeaker = useCallback((characterId: string | null) => setSpeaker(characterId ?? GM_TARGET), []);

  // 「開新桌並匯入」的開桌那一半：建好就進去，回傳新桌 id 給匯入流程顯式帶入。
  // 原桌完全不動（不回收、不改名），沿用 newTable／switchTable 的生成中防呆。
  const openTableForImport = useCallback(
    async (label: string) => {
      if (!config || chat.busy) return null;
      if (!(await canLeaveRef.current())) return null;
      const id = await invoke<string>("create_world", { name: label });
      setWorlds(await invoke<WorldMeta[]>("list_worlds"));
      await enterTable(id, config);
      return id;
    },
    [config, chat.busy],
  );

  const resetChatted = useCallback((worldId: string) => {
    localStorage.removeItem(chattedKey(worldId));
    setChattedSinceImport(false);
  }, []);

  // 匯入身分框、第二張卡路由、匯入收據與匯完跳出的開場白面板都在 controller 裡。
  // 掛在最後：它要吃 characters 與 cardInterface 的具名 action。chat 要的 noteChatStarted
  // 與開場白面板的關閉留在 App，否則 chat→imports→cardInterface→chat 會繞成環。
  const imports = useImportController({
    worldId: table,
    lang: language,
    castSize: characters.list.length,
    refreshCharacters: characters.refresh,
    reloadGmImage: characters.reloadGmImage,
    refreshInterfaces: cardInterface.refreshInterfaces,
    openIfDrawable: cardInterface.openIfDrawable,
    adoptTableName: adoptImportName,
    focusSpeaker,
    openTableForImport,
    resetChatted,
    refreshState: tableState.refresh,
    onError: setError,
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
    const receipts = await imports.loadReceipts(id);
    setTable(id);
    setScene(state.current_scene);
    setSceneTitles(state.scene_titles ?? {});
    setSceneLabels(state.scene_labels ?? {});
    tableState.hydrate(state.state, bindings);
    chat.hydrate(transcript);
    // 角色圖／GM 圖／玩家卡由 controller 自己的 effect 補：hydrate 先把上一桌的清掉，
    // 這裡一路同步提交，不讓 await 把 React batch 切成跨桌混合的中間畫面
    characters.hydrate(cast, appearanceIds, state.player_card_id);
    imports.hydrate(receipts);
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

  // 換桌／換幕／跳單幕閱讀都會清掉主欄的編輯畫面（enterTable 尾端 setMainView(null)），
  // 所以每個入口都要在動任何檔案之前先問未儲存——守門不能收進 enterTable，
  // 那時 create_world 之類的副作用已經發生，取消就會留下半張桌
  async function switchTable(id: string) {
    if (!config || id === table || chat.busy) return;
    if (!(await canLeaveEditor())) return;
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

  async function newTable() {
    if (!config || chat.busy) return;
    if (!(await canLeaveEditor())) return;
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

  // 一句話開桌的守門在「開對話框」這一刻，不在生成完成之後：等 AI 生完才問未儲存，
  // 玩家答取消就白花一次生成、磁碟上還多一張進不去的桌
  async function openGenerateTable() {
    if (!(await canLeaveEditor())) return;
    setGenTableOpen(true);
  }

  // AI 把綱要展開成一張真的桌之後：桌次清單重讀，直接進去新桌
  async function enterGeneratedTable(worldId: string) {
    await refreshWorlds();
    await enterTable(worldId, config!);
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

  // 換場：把目前場景公開紀錄壓成一則前情提要，寫進新場景開頭，current_scene +1
  async function advanceScene() {
    // 標題列不隨主欄畫面收起，編輯角色卡時這顆鈕照樣按得到
    if (!(await canLeaveEditor())) return;
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

  // 側欄「復原上次匯入」：逆向收據清單最後一筆，逐筆倒退。
  // 一次動到角色、卡片介面、檯面、狀態樹與世界設定五個域，留在 App 當跨域協調
  async function undoLastImport() {
    if (imports.receipts.length === 0) return;
    setError("");
    const last = imports.receipts[imports.receipts.length - 1];
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
      await imports.refreshReceipts(table);
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

  // 貼出開場白：真的落到檯面上了才收掉選擇面板（貼失敗時面板留著，玩家可改挑一則或重按）
  async function postOpening(text: string) {
    if (await chat.postOpening(text)) imports.closeOpenings();
  }

  // 「✨ 翻譯後貼出」：挑中那則已翻好就直接貼出；沒翻就先翻這一則，成功才貼出，
  // 失敗留在原地（原文仍在，原「貼出」鈕照常可按）。
  async function postTranslatedOpening(index: number) {
    if (imports.openings === null) return;
    const translated = await imports.translateOpening(index);
    if (translated !== null) await postOpening(translated);
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
  canLeaveRef.current = canLeaveEditor;

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

  // 前幕浮層點一幕＝整面換成單幕閱讀，一樣會蓋掉編輯畫面
  async function openSceneReader(n: number) {
    if (!(await canLeaveEditor())) return;
    setMainView({ kind: "scene", n });
    setActsOpen(false);
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
    // 重生完會直接進新的範例桌，等於換桌
    if (!(await canLeaveEditor())) return;
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
    return (
      <main className="container">
        {error && <ErrorNote text={error} transport={transportOf(config)} />}
      </main>
    );
  }

  const tableName = worlds.find((w) => w.id === table)?.name ?? "";

  return (
    <div className="app-shell">
      <GenerateTableDialog
        open={genTableOpen}
        onClose={() => setGenTableOpen(false)}
        onCreated={enterGeneratedTable}
      />
      <TableSidebar
        worlds={worlds}
        table={table}
        busy={chat.busy}
        renamingTable={editingName?.at === "list"}
        renameForm={renameForm}
        onStartRename={(name) => setEditingName({ at: "list", value: name })}
        onNewTable={() => void newTable()}
        onGenerateTable={() => void openGenerateTable()}
        onSwitchTable={(id) => void switchTable(id)}
        onDeleteTable={(id) => void deleteTable(id)}
        gmId={GM_TARGET}
        selectedCard={selectedCard}
        speakingCard={mainView ? "" : speaker}
        gmImage={characters.gmImage}
        player={characters.player}
        playerImage={characters.playerImage}
        playerAvatar={characters.playerAvatar}
        cast={characters.active}
        images={characters.images}
        avatars={characters.avatars}
        archived={characters.archived}
        onReorder={(ordered) => void characters.reorder(ordered)}
        onSelectGm={() => void selectGm()}
        onOpenWorldEditor={() => void openWorldEditor()}
        onOpenPlayerCard={() => void openPlayerCard()}
        onSelectCard={(id) => void selectCard(id)}
        onEditCard={(id) => void editCard(id)}
        onRestore={(id) => void characters.restore(id)}
        onRestoreAutoHidden={(id) => void characters.restoreAutoHidden(id)}
        onDeleteCharacter={(id) => void deleteCharacter(id)}
        onCreateCard={() => void openNewCard()}
        onImportFile={(file) => void imports.importFile(file)}
        canUndoImport={imports.receipts.length > 0 && !chattedSinceImport}
        onUndoImport={() => void undoLastImport()}
        onOpenSettings={() => setSettingsOpen("appearance")}
      />

      <main className="chat-main">
        <WorkspaceHeader
          tableName={tableName}
          renaming={editingName?.at === "header"}
          renameForm={renameForm}
          onStartRename={(name) => setEditingName({ at: "header", value: name })}
          showCardInterface={mainView === null && cardInterface.shellReady}
          onOpenCardInterface={() => cardInterface.open()}
          busy={chat.busy}
          hasEvents={chat.events.length > 0}
          onAdvanceScene={advanceScene}
          onExportTranscript={exportTranscript}
          scene={scene}
          onToggleActs={() => setActsOpen((open) => !open)}
        />

        {mainView === null && (hasStateBar || Object.keys(tableState.tree).length > 0) && (
          <StateBar
            fields={tableState.fields}
            tree={tableState.tree}
            jumps={tableState.jumps}
            bindings={tableState.bindings}
            editing={tableState.editing}
            onBeginEdit={tableState.beginEdit}
            onChangeEditValue={tableState.changeEditValue}
            onSave={(path, tree, value) => void tableState.save(path, tree, value)}
            onCancelEdit={tableState.cancelEdit}
            onMarkCounter={(path) => void tableState.markCounter(path)}
            onBind={(characterId, path) => void tableState.bind(characterId, path)}
            player={characters.player}
            castCount={characters.list.length}
            cast={characters.active}
          />
        )}

        <MainView
          actsOpen={actsOpen && scene > 0}
          scene={scene}
          onHideActs={() => setActsOpen(false)}
          onOpenScene={(n) => void openSceneReader(n)}
          sceneLabelOf={sceneDisplayLabel}
          world={table}
          worldName={tableName}
          sceneReading={mainView?.kind === "scene" ? mainView.n : null}
          onFork={(n) => void forkScene(n)}
          cardKind={cardView?.kind ?? null}
          cardId={cardView?.id ?? ""}
          cardName={characters.metaOf(cardView?.id ?? "")?.name ?? ""}
          editingPlayerCard={editingPlayerCard}
          nextColor={PALETTE[characters.list.length % PALETTE.length]}
          cardImage={
            editingPlayerCard
              ? (characters.playerImage ?? undefined)
              : characters.images[cardView?.id ?? ""]
          }
          cardAvatar={
            editingPlayerCard
              ? (characters.playerAvatar ?? undefined)
              : characters.avatars[cardView?.id ?? ""]
          }
          onImagesChanged={() =>
            editingPlayerCard
              ? characters.reloadPlayer(characters.player?.id ?? null)
              : characters.reloadImages()
          }
          onCardSaved={finishCardSaved}
          onPlayerCardSaved={finishPlayerCardSaved}
          onFinishRemoval={finishRemoval}
          onDeleteCharacter={deleteCharacter}
          onDeletePlayerCard={deletePlayerCard}
          onClose={() => setMainView(null)}
          leaveGuard={leaveGuard}
          config={config}
          sponsorUnlocked={sponsorUnlocked}
          onPreference={changePreference}
          onOpenAiSettings={() => setSettingsOpen("ai")}
          worldOpen={mainView?.kind === "world"}
          worldEditorRefreshKey={worldEditorRefreshKey}
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
            await imports.refreshReceipts(table);
          }}
          playView={
            <PlayView
              onboarding={<Onboarding config={config} onSaved={setConfig} />}
              sceneLabel={sceneDisplayLabel(scene)}
              events={chat.events}
              metaOf={characters.metaOf}
              generating={chat.generating}
              generatingMeta={generatingMeta}
              streamText={chat.streamText}
              busy={chat.busy}
              canRestore={chat.canRestore}
              onRestoreUndone={() => void chat.restoreUndone()}
              canUndoScene={canUndoScene}
              onRegenerateSummary={() => void regenerateSummary()}
              onRevertScene={() => void revertScene()}
              awayTooLong={chat.awayTooLong}
              speaker={speaker}
              gmTargeted={gmTargeted}
              targetName={targetName}
              targetColor={gmTargeted ? GM_COLOR : (characters.metaOf(speaker)?.color ?? "#888888")}
              targetImage={gmTargeted ? characters.gmImage : (characters.avatars[speaker] ?? null)}
              targetEmoji={characters.metaOf(speaker)?.avatar ?? "🎭"}
              onClearTarget={() => setSpeaker("")}
              input={chat.input}
              onInputChange={chat.setInput}
              castEmpty={characters.active.length === 0}
              onSubmit={chat.send}
              requestReplyLabel={requestReplyLabel}
              onUndoLast={() => void chat.undoLast()}
              onRequestReply={() => void chat.replyFromTarget()}
              onGmNarrate={chat.gmNarrate}
              onGmAdvance={chat.gmAdvance}
            />
          }
        />
        {error && <ErrorNote text={error} transport={transportOf(config)} />}
      </main>

      {/* 卡片自帶介面整面取代對話；殼本身已含敘事畫面，不用再疊聊天記錄；且只在遊玩畫面出現——
          切去編輯畫面時 mainView 不再是 null，這裡直接不渲染，覆蓋層就跟著消失，不用另外清 cardUiOpen */}
      {mainView === null && cardInterface.uiOpen && cardInterface.shellReady && (
        <CardInterfaceOverlay
          generatingName={
            chat.generating === null
              ? null
              : chat.generating.kind === "narration"
                ? "GM"
                : (generatingMeta?.name ?? "GM")
          }
          shellDoc={cardInterface.shellDoc}
          shellKey={cardInterface.shellKey}
          onClose={() => cardInterface.close()}
        />
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

      <ImportDialogs
        busy={chat.busy}
        choice={imports.choice}
        onAnswerChoice={(answer) => void imports.answerChoice(answer)}
        route={imports.route}
        onAnswerRoute={(answer) => void imports.answerRoute(answer)}
        openings={imports.openings}
        expanded={imports.expanded}
        translationState={imports.transState}
        translations={imports.translations}
        translateAllBusy={imports.transAllBusy}
        tier={imports.transTier}
        onSetTier={imports.setTransTier}
        tierModels={imports.tierModels}
        onSetExpanded={imports.setExpanded}
        onCloseOpenings={imports.closeOpenings}
        onTranslateAll={() => void imports.translateAllOpenings()}
        onPostOpening={(text) => void postOpening(text)}
        onTranslateAndPost={(index) => void postTranslatedOpening(index)}
        onRetranslate={(index) => void imports.translateOpening(index, true)}
      />
    </div>
  );
}

export default App;
