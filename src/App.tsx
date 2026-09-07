import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm, message as showMessage } from "@tauri-apps/plugin-dialog";
import { detectLang, Lang, normalizeLang, t } from "./i18n";
import { isCharacterHidden } from "./features/characters/character-visibility";
import { prefetchModelCatalogs } from "./features/ai-connection/model-catalog-store";
import {
  AppConfig,
  SceneLabel,
  TranscriptEvent,
  WorldMeta,
  WorldState,
} from "./shared/contracts/backend-contracts";
import { CharacterMeta } from "./features/characters/card-model";
import { useAppPreferencesController } from "./controllers/useAppPreferencesController";
import { useCardInterfaceController } from "./controllers/useCardInterfaceController";
import { useCharacterController } from "./controllers/useCharacterController";
import { useChatController } from "./controllers/useChatController";
import { useImportController } from "./controllers/useImportController";
import { useSceneActions } from "./controllers/useSceneActions";
import { loadBranchBindings, useTableStateController } from "./controllers/useTableStateController";
import {
  GM_TARGET,
  useWorkspaceNavigationController,
} from "./controllers/useWorkspaceNavigationController";
import { AppDialogs } from "./views/AppDialogs";
import { AppWorkspace, type EditingTableName } from "./views/AppWorkspace";
import { ErrorNote } from "./views/atoms";
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

// 這桌向 AI 發過對話請求了沒（每桌一把）。開演之後復原＝把演到一半的角色卡連同後續編輯一起刪掉，
// 所以按鈕要收起來；記在瀏覽器端，重開 app 也不該讓它又冒出來讓人誤按。
const chattedKey = (worldId: string) => `chatted_since_import:${worldId}`;

function App() {
  const [worlds, setWorlds] = useState<WorldMeta[]>([]);
  // table 存桌 id；顯示名一律經 tableName（見下）從 worlds 查
  const [table, setTable] = useState("");
  const [scene, setScene] = useState(0);
  const [sceneTitles, setSceneTitles] = useState<Record<string, string>>({});
  const [sceneLabels, setSceneLabels] = useState<Record<string, SceneLabel>>({});
  // 改桌名可從兩處進入：主欄標題（header）與側欄目前桌那一列（list）；at 決定輸入框長在哪
  const [editingName, setEditingName] = useState<EditingTableName>(null);
  // false＝關閉；字串＝開啟並落在該分頁（生圖對話框的「AI 連線設定」鈕直開 ai 分頁）
  const [settingsOpen, setSettingsOpen] = useState<false | "appearance" | "ai">(false);
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

  const {
    config,
    setConfig,
    sponsorUnlocked,
    setSponsorUnlocked,
    language,
    changePreference,
    markCliConnectedFromChat,
    transport,
    currentConfigRef,
  } = useAppPreferencesController({ onError: setError });

  // 狀態列／狀態樹：平欄、樹、跳動記號、分支指認與編輯中的那一格都在 controller 裡。
  // 掛在 error 之後：注入的 onError 就是 setError（useState 的 setter，identity 穩定）
  const tableState = useTableStateController({ worldId: table, onError: setError });

  // 角色名單、本幕出場集合、玩家卡與角色圖／GM 圖三份快取都在 controller 裡。
  const characters = useCharacterController({ worldId: table, onError: setError });

  const navigation = useWorkspaceNavigationController({ worldId: table, characters, onError: setError });
  const {
    canLeaveRef,
    speaker,
    setSpeaker,
    mainView,
    setMainView,
    setActsOpen,
    gmTargeted,
    canLeaveEditor,
  } = navigation;

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

  // 匯完把對話目標指過去；null＝指到 GM
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

  // 匯入身分框、第二張卡路由框、匯入收據與匯完跳出的開場白面板都在 controller 裡。
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

  const tableName = worlds.find((w) => w.id === table)?.name ?? "";
  const sceneActions = useSceneActions({
    worldId: table,
    scene,
    sceneTitles,
    sceneLabels,
    tableName,
    chat,
    config,
    canLeaveEditor,
    enterTable,
    closeMainView: () => setMainView(null),
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
    // GM 卡空不空：世界設定有字或世界書有條目都算有料，兩邊全空才把預設對象讓給第一張角色卡
    const [worldMd, worldbook] = await Promise.all([
      invoke<string>("read_world_md", { worldId: id }).catch(() => ""),
      invoke<unknown[]>("read_worldbook", { worldId: id }).catch(() => []),
    ]);
    const gmHasContent = worldMd.trim().length > 0 || worldbook.length > 0;
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
    // GM 有世界設定或世界書就由它起頭；GM 全空才退回第一張可見角色卡（隱藏區與本幕未出場的
    // auto_hidden 不算，跟側欄主區顯示一致），連角色都沒有時仍指 GM，免得輸入框鎖著沒人接
    setSpeaker(
      gmHasContent
        ? GM_TARGET
        : (cast.find((character) => !isCharacterHidden(character, appearanceIds))?.id ?? GM_TARGET),
    );
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

  async function refreshAfterEntryConverted() {
    await characters.refresh();
  }

  // AI 卡重構套用一次動到角色、玩家卡、介面殼、狀態樹與收據；這條跨域刷新刻意留在 root。
  async function refreshAfterRefactorApplied() {
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
  }

  // 設定頁改語言時：既有範例桌內容還是舊語言，問一次要不要用新語言重生（答過就記住，之後改語言不再問）
  async function changeSettingPreference(key: string, value: unknown) {
    const before = normalizeLang(currentConfigRef.current?.preferences["language"]);
    const asked = currentConfigRef.current?.preferences["sample_regen_asked"] === true;
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
    const current = currentConfigRef.current;
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
        {error && <ErrorNote text={error} transport={transport} />}
      </main>
    );
  }

  return (
    <div className="app-shell">
      <AppWorkspace
        worlds={worlds}
        table={table}
        tableName={tableName}
        scene={scene}
        editingName={editingName}
        setEditingName={setEditingName}
        hasStateBar={hasStateBar}
        chattedSinceImport={chattedSinceImport}
        worldEditorRefreshKey={worldEditorRefreshKey}
        config={config}
        sponsorUnlocked={sponsorUnlocked}
        error={error}
        transport={transport}
        characters={characters}
        chat={chat}
        tableState={tableState}
        cardInterface={cardInterface}
        imports={imports}
        navigation={navigation}
        sceneActions={sceneActions}
        onRenameTable={renameTable}
        onNewTable={newTable}
        onGenerateTable={openGenerateTable}
        onSwitchTable={switchTable}
        onDeleteTable={deleteTable}
        onUndoImport={undoLastImport}
        onOpenSettings={setSettingsOpen}
        onPreference={changePreference}
        onConfigSaved={setConfig}
        onEntryConverted={refreshAfterEntryConverted}
        onRefactorApplied={refreshAfterRefactorApplied}
      />

      <AppDialogs
        genTableOpen={genTableOpen}
        onCloseGenerateTable={() => setGenTableOpen(false)}
        onGeneratedTable={enterGeneratedTable}
        settingsOpen={settingsOpen}
        config={config}
        onConfigSaved={setConfig}
        onSettingPreference={changeSettingPreference}
        sponsorUnlocked={sponsorUnlocked}
        onSponsorUnlocked={() => setSponsorUnlocked(true)}
        onCloseSettings={() => setSettingsOpen(false)}
        currentWorld={table}
        regenOpen={regenAsk !== null}
        onAnswerRegen={answerRegen}
        chatBusy={chat.busy}
        imports={imports}
        onPostOpening={postOpening}
        onTranslateAndPost={postTranslatedOpening}
      />
    </div>
  );
}

export default App;
