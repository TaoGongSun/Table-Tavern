// 卡片介面 controller：這桌各卡的介面腳本、AI 重構產的殼、覆蓋層開關，
// 以及殼的組裝與沙盒 postMessage 往返。所有權從 App() 搬過來，行為與依賴陣列照舊。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  buildShellDocument,
  findShell,
  sanitizeCardStorage,
  type CardInterface,
  type CardStorage,
} from "../interface-card";
import { fillShellPlaceholders, fillSkeletonPlaceholders, type StateNode } from "../refactor-shell";
import { type TranscriptEvent } from "../backend-contracts";

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

export interface CardInterfaceController {
  /** 覆蓋層開著沒 */
  uiOpen: boolean;
  /** 這桌畫得出殼：頭上那顆「開啟卡片介面」鈕與覆蓋層都靠它決定出不出現 */
  shellReady: boolean;
  /** 殼的沙盒 HTML；null＝這桌沒殼 */
  shellDoc: string | null;
  /** 殼內容指紋，當 iframe 的 key */
  shellKey: string;
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
  tableTree: Record<string, StateNode>;
  submitText: (text: string) => Promise<void>;
}): CardInterfaceController {
  const { worldId, events, tableTree, submitText } = input;
  // 這桌各卡的介面腳本（DRM／雲端載入器卡沒有腳本，不進這份清單）；面板是選配功能，讀失敗就當沒有
  const [cardInterfaces, setCardInterfaces] = useState<CardInterface[]>([]);
  // AI 重構套用介面規則時可能順便產的靜態渲染殼；沒重構過或那次沒產殼就是 null，退回卡片自帶殼／event.raw 找殼
  const [refactorShell, setRefactorShell] = useState<string | null>(null);
  // 桌面玩法標記（refactor-mode-split）："characters"＝玩家選了多角色對話，這桌的卡片介面
  // 全面停用（按鈕不出現、掃 raw 的 fallback 不啟動）；null＝沒重構過或介面優先，照舊。
  // undefined＝還不知道（載入中或讀取失敗）、null＝確定沒標記；未知一律先不顯示殼
  // （fail-closed），角色桌才不會在切桌瞬間或讀取失敗時閃出介面 fallback。
  const [tableMode, setTableMode] = useState<string | null | undefined>(undefined);
  const [cardUiOpen, setCardUiOpen] = useState(false);

  // 切桌重問這桌各卡的介面腳本；先清空避免上一桌的介面殼閃現，讀失敗就當這桌沒有
  useEffect(() => {
    setCardInterfaces([]);
    if (!worldId) return;
    let stale = false;
    invoke<CardInterface[]>("card_interfaces", { worldId })
      .then((list) => {
        if (!stale) setCardInterfaces(list);
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [worldId]);

  // 切桌重問這桌的 AI 重構介面殼與玩法標記；殼讀失敗當這桌沒有，標記讀失敗維持未知不顯示殼
  useEffect(() => {
    setRefactorShell(null);
    setTableMode(undefined);
    if (!worldId) return;
    let stale = false;
    invoke<string | null>("refactor_interface_shell", { worldId })
      .then((shell) => {
        if (!stale) setRefactorShell(shell);
      })
      .catch(() => {});
    invoke<string | null>("refactor_table_mode", { worldId })
      .then((mode) => {
        if (!stale) setTableMode(mode);
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [worldId]);

  // 目前要顯示的卡片介面殼：AI 重構產過介面產物就優先用它，沒有才退回既有「近 10 則掃 event.raw」路徑。
  // 重構產物兩種：整頁 HTML（舊制殼，狀態樹填值直接顯示）；XML 骨架（照搬卡的每回合輸出格式，
  // 填值後要過卡自己的顯示腳本 regex＋模板才是畫面，`{{本回合.正文}}` 吃最新一則 GM 訊息正文）。
  const cardInterfaceShell = useMemo(() => {
    // 角色優先桌：介面產物一律不建不顯示（refactor-mode-split 拍板）——重構殼、卡片自帶殼、
    // 掃 raw 的 fallback 整組短路，永遠沒有殼。標記還沒讀回（undefined）也先不顯示，
    // 未知就放行會在角色桌切桌瞬間閃出介面。
    if (tableMode === undefined || tableMode === "characters") return null;
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
  }, [tableMode, refactorShell, tableTree, events, cardInterfaces]);

  const cardShellReady = cardInterfaceShell !== null;

  // 殼的沙盒包裝與內容指紋：指紋當 iframe key，殼一換整支 iframe 重掛——初始掛載必然載入
  // srcdoc，不依賴 WebKit 對 srcDoc 屬性更新／load 事件的行為（雙緩衝翻面機制在 WKWebView
  // 上塞殼與翻面都不可靠，三次卡片介面空白事故後整台拆除，換單 iframe 直繪）。
  // 存下的卡片設定在這裡讀進殼。刻意不進依賴：卡片一存設定就重算 doc 的話，srcdoc 跟著換，
  // 玩家拉個字級就整支 iframe 重繪閃白——殼本來就要重掛的時候（殼變了）才順手帶上最新的一份。
  const cardShellDoc = useMemo(
    () => (cardInterfaceShell === null ? null : buildShellDocument(cardInterfaceShell, readCardStorage(worldId))),
    [cardInterfaceShell, worldId],
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
    const list = await invoke<CardInterface[]>("card_interfaces", { worldId: id }).catch(
      () => [] as CardInterface[],
    );
    setCardInterfaces(list);
    return list;
  }, []);

  const refreshShell = useCallback(async (id: string) => {
    // 殼與玩法標記一起刷新：套用重構後呼叫端只叫這一支，characters 桌立刻停用介面
    const [shell, mode] = await Promise.all([
      invoke<string | null>("refactor_interface_shell", { worldId: id }).catch(() => null),
      invoke<string | null>("refactor_table_mode", { worldId: id }).catch(() => undefined),
    ]);
    setRefactorShell(shell);
    setTableMode(mode);
    return shell;
  }, []);

  // 匯入完畫得出來就直接打開一次：這類卡的開場本來就是一整頁畫面，
  // 玩家不主動點按鈕不會知道有這東西（聊天裡只看得到孤零零一句「请选择你的身份」）
  const openIfDrawable = useCallback((list: CardInterface[]) => {
    if (findShell(list, list.map((card) => card.opening)) !== null) setCardUiOpen(true);
  }, []);

  return useMemo(
    () => ({
      uiOpen: cardUiOpen,
      shellReady: cardShellReady,
      shellDoc: cardShellDoc,
      shellKey: cardShellKey,
      open,
      close,
      refreshInterfaces,
      refreshShell,
      openIfDrawable,
    }),
    [
      cardUiOpen,
      cardShellReady,
      cardShellDoc,
      cardShellKey,
      open,
      close,
      refreshInterfaces,
      refreshShell,
      openIfDrawable,
    ],
  );
}
