// 匯入 controller：角色卡／世界書的整條匯入流程（身分框、第二張卡路由、開新桌並匯）、
// 這桌的匯入收據，以及匯完跳出的開場白選擇面板與它的翻譯。
// 所有權從 App() 搬過來，行為與依賴陣列照舊。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message as showMessage } from "@tauri-apps/plugin-dialog";
import { Lang, t } from "../i18n";
import { decideImportRoute } from "../import-routing";
import { type WorldbookEntry } from "../backend-contracts";
import { PALETTE, type CharacterMeta } from "../card-model";
import { type CardInterface } from "../interface-card";

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
export interface ImportReceiptSummary {
  kind: "character" | "worldbook";
  label: string;
  timestamp: string;
  character_id?: string | null;
}

function worldbookImportedMessage(book: WorldbookImport) {
  return (
    t("worldbookImportDone", { n: book.imported }) +
    (book.skipped > 0 ? t("worldbookDuplicatesSkipped", { d: book.skipped }) : "")
  );
}

/** 待作答的第二張卡路由：身分已定、資料原樣留著，等玩家在框裡選匯哪張桌 */
export interface PendingImportRoute {
  data: number[];
  identity: "character" | "worldbook";
  label: string;
  route: "ask" | "merge_worldbook";
}

/** 待作答的匯入身分：data 原樣留著給兩條路徑共用，booksFirst＝主按鈕指世界書 */
export interface PendingImportChoice {
  data: number[];
  name: string;
  booksFirst: boolean;
}

/** 開場白面板的逐則翻譯狀態，key＝該則在清單裡的索引 */
export type OpeningTranslationState = Record<number, "translating" | "done" | "error">;

export interface ImportController {
  /** 這桌的匯入收據摘要：側欄「復原上次匯入」按鈕靠它決定出不出現 */
  receipts: ImportReceiptSummary[];
  /** 匯入身分框：等玩家在三鍵框挑一種，null＝沒開 */
  choice: PendingImportChoice | null;
  /** 第二張卡路由框：null＝沒開 */
  route: PendingImportRoute | null;
  /** 匯完跳出的開場白清單；null＝面板沒開 */
  openings: string[] | null;
  /** 面板裡展開的那一則（一次只展開一條） */
  expanded: number | null;
  setExpanded: (index: number | null) => void;
  /** 逐則翻譯狀態 */
  transState: OpeningTranslationState;
  /** 「全部翻譯」跑著沒 */
  transAllBusy: boolean;
  closeOpenings: () => void;
  /** 換桌：這桌的收據整份換掉（只做 state commit，不得有 await） */
  hydrate: (receipts: ImportReceiptSummary[]) => void;
  /** 換桌前先把收據讀好，讀完才進 hydrate 的同步提交區 */
  loadReceipts: (worldId: string) => Promise<ImportReceiptSummary[]>;
  /** 匯完（或復原掉一筆）後重讀收據 */
  refreshReceipts: (worldId: string) => Promise<void>;
  /** 側欄「匯入卡」選了檔：探測身分後分流 */
  importFile: (file: File) => Promise<void>;
  answerChoice: (choice: "character" | "worldbook" | "cancel") => Promise<void>;
  answerRoute: (choice: "this_table" | "new_table" | "cancel") => Promise<void>;
  /** 翻好的那一則；null＝翻譯失敗或面板已關 */
  translateOpening: (index: number) => Promise<string | null>;
  translateAllOpenings: () => Promise<void>;
}

export function useImportController(input: {
  worldId: string;
  /** 讀開場白與翻譯都要帶語系 */
  lang: Lang;
  /** 含隱藏區的角色數：新卡的陣營色照名單長度輪替 */
  castSize: number;
  refreshCharacters: (worldId?: string) => Promise<CharacterMeta[]>;
  reloadGmImage: (worldId?: string) => Promise<void>;
  refreshInterfaces: (worldId: string) => Promise<CardInterface[]>;
  openIfDrawable: (list: CardInterface[]) => void;
  /** 匯入成功後把還掛自動名的桌改成卡名／書名 */
  adoptTableName: (name: string | null | undefined) => Promise<void>;
  /** 匯完把對話目標指過去；null＝指到 GM */
  focusSpeaker: (characterId: string | null) => void;
  /** 開一張新桌並進去，回傳新桌 id；null＝現在開不了（沒 config 或正在生成） */
  openTableForImport: (label: string) => Promise<string | null>;
  /** 剛匯入＝又回到「還沒開演」的狀態，把這桌的開演記號清掉 */
  resetChatted: (worldId: string) => void;
  onError: (message: string) => void;
}): ImportController {
  const {
    worldId,
    lang,
    castSize,
    refreshCharacters,
    reloadGmImage,
    refreshInterfaces,
    openIfDrawable,
    adoptTableName,
    focusSpeaker,
    openTableForImport,
    resetChatted,
    onError,
  } = input;

  // 這桌的匯入收據摘要：非空、且還沒開始跟 AI 對話，才顯示「復原上次匯入」按鈕
  const [receipts, setReceipts] = useState<ImportReceiptSummary[]>([]);
  // 匯入身分框：等玩家在三鍵框挑一種，data 原樣留著給兩條路徑共用；
  // booksFirst＝主按鈕指世界書（探測結果只用來算這個，算完就不必留著）
  const [choice, setChoice] = useState<PendingImportChoice | null>(null);
  // 第二張卡路由框：身分已定、桌上已有匯入紀錄才會跳出來；ask＝一般第二張卡、merge_worldbook＝第二本世界書會合成一本
  const [route, setRoute] = useState<PendingImportRoute | null>(null);
  const [openings, setOpenings] = useState<string[] | null>(null);
  // 一次只展開一條：面板不長，攤開多條反而找不到自己在看哪一段
  const [expanded, setExpanded] = useState<number | null>(null);
  // 開場白翻譯：逐則狀態＋「全部翻譯」是否在跑；abort ref 給 modal 一關就停止後續翻譯呼叫用
  // （純 ref 而非 state：序列迴圈中途要讀到「使用者剛剛關掉視窗」，不能等下一次 render）
  const [transState, setTransState] = useState<OpeningTranslationState>({});
  const [transAllBusy, setTransAllBusy] = useState(false);
  const transAbort = useRef(false);
  // openings 一變成 null（不管哪個按鈕關的）就中止：不必在每個關閉入口各補一次旗標
  useEffect(() => {
    if (openings === null) transAbort.current = true;
  }, [openings]);

  const loadReceipts = useCallback(
    (worldId: string) =>
      invoke<ImportReceiptSummary[]>("list_import_receipts", { worldId }).catch(() => []),
    [],
  );

  const hydrate = useCallback((receipts: ImportReceiptSummary[]) => {
    setReceipts(receipts);
  }, []);

  const refreshReceipts = useCallback(
    async (worldId: string) => {
      setReceipts(await loadReceipts(worldId));
      // 剛匯入（或剛復原一筆）＝又回到「還沒開演」的狀態，按鈕重新給
      resetChatted(worldId);
    },
    [loadReceipts, resetChatted],
  );

  const closeOpenings = useCallback(() => setOpenings(null), []);

  // 兩條匯入路徑共用：畫得出來就告訴玩家在哪開並直接開一次，解不開的講清楚是哪一種
  // （加密卡、介面存在別人網站上的雲端載入器卡）。沒有介面的卡什麼都不說。
  const tellAboutInterface = useCallback(
    async (worldId: string, characterId: string) => {
      const interfaces = await refreshInterfaces(worldId);
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
      openIfDrawable(interfaces);
    },
    [refreshInterfaces, openIfDrawable],
  );

  // 兩條匯入路徑共用。一律貼成旁白而不是角色發言——開場白不一定是那個角色說的話，
  // 也常是場景或角色本身的描寫。主開場白常是使用說明（真正的劇情藏在備用開場白），
  // 所以列全部讓玩家挑。直接讀匯入檔，不建卡也拿得到
  const offerOpeningLine = useCallback(
    async (worldId: string, data: number[]) => {
      const list = await invoke<string[]>("card_openings", { worldId, data, lang });
      if (list.length === 0) return;
      setExpanded(null);
      setTransState({});
      transAbort.current = false;
      setOpenings(list);
    },
    [lang],
  );

  // worldId 顯式帶入（不吃這桌的 worldId）：開新桌並匯入時外層那個當下還是舊桌值。
  // adoptName 預設 true；開新桌路徑傳 false——新桌從建立那刻就已經用卡名命名，不必再改一次。
  const importAsCharacter = useCallback(
    async (worldId: string, data: number[], adoptName = true) => {
      const { meta, book } = await invoke<CharacterImport>("import_character", {
        worldId,
        data,
        color: PALETTE[castSize % PALETTE.length],
      });
      await refreshCharacters(worldId);
      focusSpeaker(meta.id);
      if (adoptName) await adoptTableName(meta.name);
      await refreshReceipts(worldId);
      // 卡片隨身的世界書條目也要報數，跟世界書路徑講一樣的話
      if (book.imported > 0) await showMessage(worldbookImportedMessage(book), { title: t("importCard") });
      await offerOpeningLine(worldId, data);
      await tellAboutInterface(worldId, meta.id);
    },
    [castSize, refreshCharacters, focusSpeaker, adoptTableName, refreshReceipts, offerOpeningLine, tellAboutInterface],
  );

  // 世界書匯入共用流程：側欄按鈕分流出的純世界書檔、三鍵對話框選了世界書都走這裡。
  // worldId 顯式帶入、adoptName 預設 true，理由同 importAsCharacter。
  const importAsWorldbook = useCallback(
    async (worldId: string, data: number[], label: string, adoptName = true) => {
      const book = await invoke<WorldbookImport>("import_worldbook", { worldId, data, label });
      // 匯的是 PNG 卡：後端已把整張圖存成 GM 卡的圖，這裡讀回來讓側欄立刻換掉書本圖
      await reloadGmImage(worldId);
      await showMessage(worldbookImportedMessage(book), { title: t("importCard") });
      // 世界書更容易不知道怎麼開始：匯完一律把對話目標指到 GM
      focusSpeaker(null);
      if (adoptName) await adoptTableName(label);
      await refreshReceipts(worldId);
      await offerOpeningLine(worldId, data);
      // 這桌等級的介面殼 character_id 是空字串（角色卡的是那張卡的 id）
      await tellAboutInterface(worldId, "");
    },
    [reloadGmImage, focusSpeaker, adoptTableName, refreshReceipts, offerOpeningLine, tellAboutInterface],
  );

  // 收據為空時的保險（見 decideImportRoute）。現讀而不吃 state：世界書條目歸 WorldEditor 管，
  // 這裡沒有同步的一份。讀不到就當有——寧可多問一句，也不要把書無聲合進有內容的桌。
  const tableHasWorldbookEntries = useCallback(async () => {
    try {
      return (await invoke<WorldbookEntry[]>("read_worldbook", { worldId })).length > 0;
    } catch {
      return true;
    }
  }, [worldId]);

  // 第二張卡路由：身分已定，看這桌現況決定要不要跳提醒框。
  // direct 零打擾直接匯；ask／merge_worldbook 開框，框裡選完才真的匯（見 answerRoute）。
  const routeImport = useCallback(
    async (identity: "character" | "worldbook", data: number[], label: string) => {
      // 收據為空才問條目：那可能是收據功能之前的舊桌、手建的桌或範例桌
      const needsFallback = identity === "worldbook" && receipts.length === 0;
      const route = decideImportRoute(
        identity,
        receipts.map((receipt) => receipt.kind),
        needsFallback && (await tableHasWorldbookEntries()),
      );
      if (route === "direct") {
        if (identity === "worldbook") await importAsWorldbook(worldId, data, label);
        else await importAsCharacter(worldId, data);
        return;
      }
      setRoute({ data, identity, label, route });
    },
    [receipts, worldId, tableHasWorldbookEntries, importAsWorldbook, importAsCharacter],
  );

  // 開新桌並匯入：桌名直接用卡名／書名／檔名（pending.label），create_world 回傳的新 id
  // 全程顯式帶入（不靠這桌的 worldId，切桌當下它還是舊值），原桌完全不動（不回收、不改名）。
  // adoptTableName 不需要跑：新桌從一開始就用 label 命名。
  const openNewTableAndImport = useCallback(
    async (pending: { data: number[]; identity: "character" | "worldbook"; label: string }) => {
      const id = await openTableForImport(pending.label);
      if (id === null) return;
      if (pending.identity === "worldbook") {
        await importAsWorldbook(id, pending.data, pending.label, false);
      } else {
        await importAsCharacter(id, pending.data, false);
      }
    },
    [openTableForImport, importAsWorldbook, importAsCharacter],
  );

  // 匯入 SillyTavern 角色卡（V2 PNG 或 JSON）：讀 bytes 交後端探測，依探測結果分流——
  // 純世界書、純角色卡都零詢問直接判定身分（還要再過第二張卡路由）；
  // 角色與世界書兩種身分都有料才彈三鍵對話框問玩家要哪個，答完一樣過路由。
  const importFile = useCallback(
    async (file: File) => {
      onError("");
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
          setChoice({ data, name: probe.name, booksFirst: looksLikeWorldbook(probe) });
          return;
        }
        // 解析失敗：照舊走角色路徑，讓後端報原本的格式錯誤，不算第二張卡場景，不過路由
        await importAsCharacter(worldId, data);
      } catch (reason) {
        onError(String(reason));
      }
    },
    [routeImport, importAsCharacter, worldId, onError],
  );

  // 三鍵對話框的作答：取消什麼都不做，另外兩個選項答出身分後都要過第二張卡路由
  const answerChoice = useCallback(
    async (answer: "character" | "worldbook" | "cancel") => {
      const pending = choice;
      setChoice(null);
      if (!pending || answer === "cancel") return;
      onError("");
      try {
        await routeImport(answer, pending.data, pending.name);
      } catch (reason) {
        onError(String(reason));
      }
    },
    [choice, routeImport, onError],
  );

  // 路由框作答：取消什麼都不做；匯進這桌走現行匯入函式（第二本世界書走同一條，後端會接在既有條目後面並去重）；
  // 開新桌並匯入另開一桌後再匯，見 openNewTableAndImport
  const answerRoute = useCallback(
    async (answer: "this_table" | "new_table" | "cancel") => {
      const pending = route;
      setRoute(null);
      if (!pending || answer === "cancel") return;
      onError("");
      try {
        if (answer === "this_table") {
          if (pending.identity === "worldbook") {
            await importAsWorldbook(worldId, pending.data, pending.label);
          } else {
            await importAsCharacter(worldId, pending.data);
          }
        } else {
          await openNewTableAndImport(pending);
        }
      } catch (reason) {
        onError(String(reason));
      }
    },
    [route, worldId, importAsWorldbook, importAsCharacter, openNewTableAndImport, onError],
  );

  // 開場白翻譯：單則呼叫 translate_opening（走 fast 檔，失敗退 GM 檔，見 lib.rs），
  // 兩顆翻譯鈕共用。已經 done 的直接回傳目前內容，不重打；modal 關閉中途 abort 就不再
  // 動 state（視窗都不在了，setState 也只是白費）。
  const translateOpening = useCallback(
    async (index: number): Promise<string | null> => {
      if (openings === null) return null;
      if (transState[index] === "done") return openings[index];
      const text = openings[index];
      setTransState((previous) => ({ ...previous, [index]: "translating" }));
      try {
        const translated = await invoke<string>("translate_opening", { worldId, text, lang });
        if (transAbort.current) return null;
        setOpenings((previous) =>
          previous === null ? previous : previous.map((item, itemIndex) => (itemIndex === index ? translated : item)),
        );
        setTransState((previous) => ({ ...previous, [index]: "done" }));
        return translated;
      } catch (reason) {
        if (transAbort.current) return null;
        setTransState((previous) => ({ ...previous, [index]: "error" }));
        onError(String(reason));
        return null;
      }
    },
    [openings, transState, worldId, lang, onError],
  );

  // 「✨ 全部翻譯」：逐則序列翻譯，不擋操作（沒鎖住 modal 其他按鈕）；modal 一關（abort
  // 旗標翻真）就停止發下一則呼叫，省下不會有人看到的 AI 額度。
  const translateAllOpenings = useCallback(async () => {
    if (openings === null || transAllBusy) return;
    setTransAllBusy(true);
    transAbort.current = false;
    for (let index = 0; index < openings.length; index += 1) {
      if (transAbort.current) break;
      await translateOpening(index);
    }
    setTransAllBusy(false);
  }, [openings, transAllBusy, translateOpening]);

  return useMemo(
    () => ({
      receipts,
      choice,
      route,
      openings,
      expanded,
      setExpanded,
      transState,
      transAllBusy,
      closeOpenings,
      hydrate,
      loadReceipts,
      refreshReceipts,
      importFile,
      answerChoice,
      answerRoute,
      translateOpening,
      translateAllOpenings,
    }),
    [
      receipts,
      choice,
      route,
      openings,
      expanded,
      transState,
      transAllBusy,
      closeOpenings,
      hydrate,
      loadReceipts,
      refreshReceipts,
      importFile,
      answerChoice,
      answerRoute,
      translateOpening,
      translateAllOpenings,
    ],
  );
}
