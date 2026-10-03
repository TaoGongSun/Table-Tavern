// 卡片介面 controller：這桌各卡的介面腳本、AI 重構產的骨架、覆蓋層開關，
// 以及殼的組裝與沙盒 postMessage 往返。所有權從 App() 搬過來，行為與依賴陣列照舊。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  buildShellDocument,
  findShell,
  sanitizeCardStorage,
  type CardInterface,
  type CardStorage,
} from "./interface-card";
import { pickCardShell } from "./card-shell-route";
import { buildCardChat, type CardChat } from "./card-chat-shim";
import { type CardMvu, type StateTree } from "./card-mvu-shim";
import { type TranscriptEvent } from "../../shared/contracts/backend-contracts";

// 短指紋（djb2）：card-interface iframe 的 key 用，內容一換 key 就換。
function fingerprint(text: string): string {
  let hash = 5381;
  for (let i = 0; i < text.length; i++) hash = ((hash << 5) + hash + text.charCodeAt(i)) | 0;
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

export interface CardInterfaceController {
  /** 覆蓋層開著沒 */
  uiOpen: boolean;
  /** 這桌畫得出殼：頭上那顆「開啟卡片介面」鈕與覆蓋層都靠它決定出不出現 */
  shellReady: boolean;
  /** 介面由 App 接管（有重構骨架、不是角色優先桌；與後端 gm_turn_format 同一依據）：模型只回報
   *  UpdateVariable、不寫 state 圍欄，頂部狀態欄不顯示 */
  interfaceTakeover: boolean;
  /** 殼的沙盒 HTML；null＝這桌沒殼 */
  shellDoc: string | null;
  /** 桌別＋殼＋本樓的指紋，當 iframe 的 key，也是讀訊息推送的 token */
  shellKey: string;
  /** 本場讀訊息快照；覆蓋層在它變動與 iframe load 時推給沙盒 */
  chat: CardChat | null;
  /** MVU 變數快照（null＝這桌沒有 MVU 卡）；與 chat 一起推 */
  mvu: CardMvu | null;
  open: () => void;
  close: () => void;
  /** 重問這桌各卡的介面腳本，並把清單回給呼叫端接著判斷 */
  refreshInterfaces: (worldId: string) => Promise<CardInterface[]>;
  /** 重問這桌的 AI 重構介面殼 */
  refreshShell: (worldId: string) => Promise<string | null>;
  /** 匯入完畫得出來就直接打開一次 */
  openIfDrawable: (list: CardInterface[]) => void;
}

export function useCardInterfaceController(input: {
  worldId: string;
  events: TranscriptEvent[];
  /** 目前狀態樹（含面板手動改值）：MVU 卡的活樓讀它 */
  tree: StateTree;
  /** 這桌玩家名（沒有就用語系稱呼）：MVU 變數代換 {{user}} */
  userName: string;
  submitText: (text: string) => Promise<void>;
}): CardInterfaceController {
  const { worldId, events, tree, userName, submitText } = input;
  // 這桌各卡的介面腳本（DRM／雲端載入器卡沒有腳本，不進這份清單）；面板是選配功能，讀失敗就當沒有
  const [cardInterfaces, setCardInterfaces] = useState<CardInterface[]>([]);
  // AI 重構接管介面時產的骨架（卡每回合輸出格式）；null＝沒有，這時照原卡畫面（卡片自帶殼／event.raw 找殼）
  const [refactorShell, setRefactorShell] = useState<string | null>(null);
  // 桌面玩法標記（refactor-mode-split）："characters"＝玩家選了多角色對話，這桌的卡片介面
  // 全面停用（按鈕不出現、掃 raw 的 fallback 不啟動）；"interface"＝有骨架先畫骨架；null＝沒重構過。
  // undefined＝還不知道（載入中或讀取失敗）、null＝確定沒標記；未知一律先不顯示殼
  // （fail-closed），角色桌才不會在切桌瞬間或讀取失敗時閃出介面 fallback。
  const [tableMode, setTableMode] = useState<string | null | undefined>(undefined);
  // 原卡欄位型別（重構套用時記下）：骨架填值時決定數字／布林的寫法
  const [valueTypes, setValueTypes] = useState<Record<string, string>>({});
  const [cardUiOpen, setCardUiOpen] = useState(false);

  // 介面腳本與殼的讀取世代號：切桌或又讀一次後，晚到的舊回應（含撤銷、套用刷新途中換桌）不寫進畫面
  const interfacesLoad = useRef(0);
  const shellLoad = useRef(0);

  // 切桌重問這桌各卡的介面腳本；先清空避免上一桌的介面殼閃現，讀失敗就當這桌沒有
  useEffect(() => {
    const mine = ++interfacesLoad.current;
    setCardInterfaces([]);
    if (!worldId) return;
    invoke<CardInterface[]>("card_interfaces", { worldId })
      .then((list) => {
        if (mine === interfacesLoad.current) setCardInterfaces(list);
      })
      .catch(() => {});
  }, [worldId]);

  // 切桌重問這桌的 AI 重構介面殼與玩法標記；殼讀失敗當這桌沒有，標記讀失敗維持未知不顯示殼
  useEffect(() => {
    const mine = ++shellLoad.current;
    const current = () => mine === shellLoad.current;
    setRefactorShell(null);
    setTableMode(undefined);
    if (!worldId) return;
    invoke<string | null>("refactor_interface_shell", { worldId })
      .then((shell) => {
        if (current()) setRefactorShell(shell);
      })
      .catch(() => {});
    invoke<string | null>("refactor_table_mode", { worldId })
      .then((mode) => {
        if (current()) setTableMode(mode);
      })
      .catch(() => {});
    setValueTypes({});
    invoke<{ mechanism?: { value_types?: Record<string, string> } }>("read_state", { worldId })
      .then((state) => {
        if (current()) setValueTypes(state?.mechanism?.value_types ?? {});
      })
      .catch(() => {});
  }, [worldId]);

  // 目前要顯示的卡片介面殼與產生它的那一樓：選路規則見 card-shell-route.ts
  const picked = useMemo(
    () =>
      pickCardShell({
        tableMode,
        refactorShell,
        events,
        cardInterfaces,
        valueTypes,
        mvu: { liveTree: tree, userName },
      }),
    [tableMode, refactorShell, events, cardInterfaces, valueTypes, tree, userName],
  );
  // doc 與 key 只依賴實際值：無關的 render（例如狀態樹變了但殼與本樓沒變）不重載 iframe
  const shell = picked?.shell ?? null;
  const currentId = picked?.current.id ?? -1;
  const currentName = picked?.current.name ?? "";
  const currentText = picked?.current.text ?? "";

  const cardShellReady = shell !== null;

  // 本場讀訊息快照：掛載時嵌進 doc，之後的變動由覆蓋層推送（本樓一律是產生殼的那段文字）
  const chat = useMemo(() => (picked === null ? null : buildCardChat(picked.floors, picked.current)), [picked]);
  const chatRef = useRef<CardChat | null>(null);
  chatRef.current = chat;
  const mvu = picked?.mvu ?? null;
  const mvuRef = useRef<CardMvu | null>(null);
  mvuRef.current = mvu;

  // 殼沒了（例如面板開著時套用了沒產殼的重構）就把面板狀態一起收掉：只靠 shellReady 擋住
  // 覆蓋層的話，之後殼再出現時面板會自己跳出來
  useEffect(() => {
    if (!cardShellReady) setCardUiOpen(false);
  }, [cardShellReady]);

  // 殼的沙盒包裝與內容指紋：指紋當 iframe key，殼一換整支 iframe 重掛——初始掛載必然載入
  // srcdoc，不依賴 WebKit 對 srcDoc 屬性更新／load 事件的行為（雙緩衝翻面機制在 WKWebView
  // 上塞殼與翻面都不可靠，三次卡片介面空白事故後整台拆除，換單 iframe 直繪）。
  // 存下的卡片設定與讀訊息快照都經 ref／讀檔帶進殼、刻意不進依賴：卡片一存設定或逐字稿一動就重算
  // doc 的話，srcdoc 跟著換，整支 iframe 重繪閃白——殼或本樓變了（key 也跟著變）才順手帶上最新的。
  // 依賴 uiOpen：面板關著時別樓有變動，重新打開時在卡片第一次執行前就嵌入最新快照
  // （覆蓋層關著時整支卸載，重開必然是新掛載）。key 涵蓋桌別：同殼同文切桌也會重掛。
  const cardShellKey = useMemo(
    () =>
      shell === null
        ? "empty"
        : fingerprint(JSON.stringify([worldId, shell, currentId, currentName, currentText])),
    [worldId, shell, currentId, currentName, currentText],
  );
  const cardShellDoc = useMemo(
    () =>
      shell === null || !cardUiOpen || chatRef.current === null
        ? null
        : buildShellDocument(shell, readCardStorage(worldId), {
            chat: chatRef.current,
            token: cardShellKey,
            mvu: mvuRef.current,
          }),
    [shell, cardShellKey, worldId, cardUiOpen],
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
      // 卡片存設定：只落到宿主存檔，不碰 state——這裡一改 state 就會連動 srcdoc 重繪
      if (data.kind === "storage") {
        writeCardStorage(worldId, data.entries);
        return;
      }
      // 焦點在沙盒 iframe 裡時的 Esc：keydown 不跨 document 冒泡，只能由墊片回報
      if (data.kind === "close") {
        setCardUiOpen(false);
        return;
      }
      if (data.kind !== "input") return;
      void submitTextRef.current(String(data.text ?? ""));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [cardUiOpen, worldId]);

  // Esc 關閉卡片介面覆蓋層；只在開著時掛，避免和其他 Esc 行為（如取消改名）互相搶。
  // 這條只管焦點還在宿主的時候；焦點進了沙盒 iframe 之後走墊片回報的 kind: "close"。
  useEffect(() => {
    if (!cardUiOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setCardUiOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cardUiOpen]);

  const open = useCallback(() => setCardUiOpen(true), []);
  const close = useCallback(() => setCardUiOpen(false), []);

  const refreshInterfaces = useCallback(async (id: string) => {
    const mine = ++interfacesLoad.current;
    const list = await invoke<CardInterface[]>("card_interfaces", { worldId: id }).catch(
      () => [] as CardInterface[],
    );
    if (mine === interfacesLoad.current) setCardInterfaces(list);
    return list;
  }, []);

  const refreshShell = useCallback(async (id: string) => {
    const mine = ++shellLoad.current;
    // 殼與玩法標記一起刷新：套用重構後呼叫端只叫這一支，characters 桌立刻停用介面
    const [shell, mode, state] = await Promise.all([
      invoke<string | null>("refactor_interface_shell", { worldId: id }).catch(() => null),
      invoke<string | null>("refactor_table_mode", { worldId: id }).catch(() => undefined),
      invoke<{ mechanism?: { value_types?: Record<string, string> } }>("read_state", { worldId: id }).catch(
        () => null,
      ),
    ]);
    if (mine !== shellLoad.current) return shell;
    setRefactorShell(shell);
    setTableMode(mode);
    setValueTypes(state?.mechanism?.value_types ?? {});
    return shell;
  }, []);

  // 匯入完畫得出來就直接打開一次：這類卡的開場本來就是一整頁畫面，
  // 玩家不主動點按鈕不會知道有這東西（聊天裡只看得到孤零零一句「请选择你的身份」）
  const openIfDrawable = useCallback((list: CardInterface[]) => {
    if (findShell(list, list.map((card) => card.opening)) !== null) setCardUiOpen(true);
  }, []);

  const interfaceTakeover = (refactorShell?.trim() ?? "") !== "" && tableMode !== "characters";

  return useMemo(
    () => ({
      uiOpen: cardUiOpen,
      interfaceTakeover,
      shellReady: cardShellReady,
      shellDoc: cardShellDoc,
      shellKey: cardShellKey,
      chat,
      mvu,
      open,
      close,
      refreshInterfaces,
      refreshShell,
      openIfDrawable,
    }),
    [
      mvu,
      cardUiOpen,
      interfaceTakeover,
      cardShellReady,
      cardShellDoc,
      cardShellKey,
      chat,
      open,
      close,
      refreshInterfaces,
      refreshShell,
      openIfDrawable,
    ],
  );
}
