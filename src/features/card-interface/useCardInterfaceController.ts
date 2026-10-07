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
import { type CardMvu, type MvuLayer, type StateTree } from "./mvu/card-mvu-shim";
import {
  createMvuWriteQueue,
  type Authority,
  type CardWriteResult,
  type Migration,
  type MvuWriteQueue,
  layerBlocked,
  parseLayerKey,
  type SettleResult,
} from "./mvu/card-mvu-write";
import { createEvalHost, parseEvalRequest, type EvalHost } from "./mvu/card-mvu-eval-host";
import { type TranscriptEvent } from "../../shared/contracts/backend-contracts";

// 短指紋（djb2）：card-interface iframe 的 key 用，內容一換 key 就換。
function fingerprint(text: string): string {
  let hash = 5381;
  for (let i = 0; i < text.length; i++) hash = ((hash << 5) + hash + text.charCodeAt(i)) | 0;
  return String(hash >>> 0);
}

/** 後端 `card_vars_state`：桌世代、這一幕是否走逐樓變數表、目前幕號 */
interface VarsState {
  generation: number;
  active: boolean;
  scene: number;
}

/** 寫入目標 key → 宿主逐字稿裡的那一則：事件 id，舊事件是 "@逐字稿位置" */
function eventOfKey(events: TranscriptEvent[], key: string): TranscriptEvent | undefined {
  if (key.startsWith("@")) {
    const event = events[Number(key.slice(1))];
    return event !== undefined && event.id === undefined ? event : undefined;
  }
  return events.find((event) => event.id === key);
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

/**
 * 寫進這桌的卡片 storage，回傳有沒有寫成。沙盒推來的快照寫不進去（配額滿等）時設定留在沙盒記憶體裡、
 * 不影響畫面；匯入網頁存檔時呼叫端看回傳值，寫不成就不能當成功。
 */
export function writeCardStorage(worldId: string | null, entries: unknown): boolean {
  const clean = worldId === null ? null : sanitizeCardStorage(entries);
  if (clean === null) return false;
  try {
    window.localStorage.setItem(CARD_STORAGE_PREFIX + worldId, JSON.stringify(clean));
    return true;
  } catch {
    return false;
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

/** 一份殼的宿主端資源：寫入佇列、值解析 Worker 與最近一次來訊的沙盒窗口 */
interface ShellHolder {
  token: string;
  queue: MvuWriteQueue;
  evals: EvalHost;
  reply: MessageEventSource | null;
}

export function useCardInterfaceController(input: {
  worldId: string;
  events: TranscriptEvent[];
  /** 目前狀態樹（含面板手動改值）：MVU 卡的活樓讀它 */
  tree: StateTree;
  /** 這桌玩家名（沒有就用語系稱呼）：MVU 變數代換 {{user}} */
  userName: string;
  /** 卡片寫入確認落檔後（主頁狀態欄要重讀） */
  onCardWrite?: () => void;
  submitText: (text: string) => Promise<void>;
  /** 卡片寫入落檔後把那一則換進逐字稿（同一物件或同 id） */
  onEventUpdated?: (previous: TranscriptEvent, next: TranscriptEvent) => void;
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

  // 卡片變數（MVU message 層）的這桌現況：桌世代、這一幕是否走逐樓表。有 MVU 卡才問；逐字稿一變就重問
  // （開場、第一回合或第一次卡寫會啟用），晚到的舊回應不寫進畫面
  const hasMvuCard = cardInterfaces.some((card) => card.mvu === true && card.unsupported === null);
  const [varsState, setVarsState] = useState<VarsState | null>(null);
  const varsLoad = useRef(0);
  const refreshVarsState = useCallback(async () => {
    const mine = ++varsLoad.current;
    if (!worldId || !hasMvuCard) {
      setVarsState(null);
      return null;
    }
    const state = await invoke<VarsState>("card_vars_state", { worldId }).catch(() => null);
    if (mine === varsLoad.current) setVarsState(state);
    return state;
  }, [worldId, hasMvuCard]);
  useEffect(() => {
    void refreshVarsState();
  }, [refreshVarsState, events]);

  // 非 message 層現況（key 與沙盒寫入目標同一套，計畫 8.7）：面板開著才讀（每次開都重讀），關掉清空；
  // 讀回前殼文件先不出（卡片第一次執行時就要讀得到值）。讀回的資料綁定（桌、桌世代、殼所屬身分）：
  // 任一項變了，舊資料立刻失去就緒資格，等符合新身分的讀回才掛載
  const [layersState, setLayersState] = useState<{ key: string; data: Record<string, MvuLayer> } | null>(null);
  const layersRef = useRef<Record<string, MvuLayer> | null>(null);
  const layersKeyRef = useRef<string | null>(null);
  const layersLoad = useRef(0);

  // 目前要顯示的卡片介面殼與產生它的那一樓：選路規則見 card-shell-route.ts
  const varsActive = varsState?.active === true;
  const varsGeneration = varsState?.generation ?? -1;
  const varsScene = varsState?.scene ?? 0;
  const picked = useMemo(
    () =>
      pickCardShell({
        tableMode,
        refactorShell,
        events,
        cardInterfaces,
        valueTypes,
        mvu: {
          liveTree: tree,
          userName,
          active: varsActive,
          generation: varsGeneration,
          scene: varsScene,
        },
      }),
    [
      tableMode,
      refactorShell,
      events,
      cardInterfaces,
      valueTypes,
      tree,
      userName,
      varsActive,
      varsGeneration,
      varsScene,
    ],
  );
  // 讀非 message 層：character 層的身分是殼實際所屬的卡（pickCardShell 決定，空字串＝不唯一、不讀）。
  // 讀取失敗不是空表：各層標成 error（沙盒讀取拋錯、寫入拒絕），重開面板重讀
  const characterId = picked?.mvu?.characterId ?? null;
  const layersKey = characterId === null ? null : JSON.stringify([worldId, varsState?.generation ?? -1, characterId]);
  const layers = layersKey !== null && layersState?.key === layersKey ? layersState.data : null;
  layersRef.current = layers;
  layersKeyRef.current = layersKey;
  useEffect(() => {
    const mine = ++layersLoad.current;
    if (!cardUiOpen || !worldId || !hasMvuCard || characterId === null || layersKey === null) {
      setLayersState(null);
      return;
    }
    const apply = (next: Record<string, MvuLayer>) => {
      if (mine !== layersLoad.current) return;
      layersRef.current = next;
      setLayersState({ key: layersKey, data: next });
    };
    invoke<{ key: string; rev: string | null; vars: string; error?: string }[]>("card_layers", {
      worldId,
      characterId: characterId === "" ? null : characterId,
    })
      .then((entries) => {
        const next: Record<string, MvuLayer> = {};
        for (const entry of entries) {
          next[entry.key] =
            entry.error === undefined
              ? { rev: entry.rev, vars: JSON.parse(entry.vars) }
              : { rev: null, vars: {}, error: entry.error };
        }
        apply(next);
      })
      .catch((reason) => {
        const error = `load-failed: ${String(reason)}`;
        const failed: Record<string, MvuLayer> = {};
        for (const key of ["chat", "global", "preset", "script:", "extension:", ...(characterId === "" ? [] : [`character:${characterId}`])]) {
          failed[key] = { rev: null, vars: {}, error };
        }
        apply(failed);
      });
  }, [cardUiOpen, worldId, hasMvuCard, characterId, layersKey]);
  // 非 message 層現況換成新版（寫入確認或被拒推回權威值）：先改 ref（下一筆寫入立刻拿得到），再換 state 推給沙盒
  const setLayer = useCallback((key: string, layer: MvuLayer) => {
    if (layersRef.current === null) return;
    const next = { ...layersRef.current, [key]: layer };
    const bound = layersKeyRef.current;
    layersRef.current = next;
    setLayersState((previous) => (previous !== null && previous.key === bound ? { key: bound, data: next } : previous));
  }, []);

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
  const pickedMvu = picked?.mvu ?? null;
  const mvu = useMemo(() => (pickedMvu === null ? null : { ...pickedMvu, layers: layers ?? {} }), [pickedMvu, layers]);
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
  // （覆蓋層關著時整支卸載，重開必然是新掛載）。key 涵蓋桌別、桌世代與幕：同殼同文切桌、整桌還原（備份帶回
  // 同 id 的事件）也會重掛，舊殼的寫入佇列跟著關掉。
  const cardShellKey = useMemo(
    () =>
      shell === null
        ? "empty"
        : fingerprint(
            JSON.stringify([worldId, varsGeneration, varsScene, shell, currentId, currentName, currentText]),
          ),
    [worldId, varsGeneration, varsScene, shell, currentId, currentName, currentText],
  );
  // MVU 卡的殼要等非 message 層讀回才出文件（hasMvuCard 與 mvu 非 null 才有非 message 層可讀）
  const layersReady = mvu === null || layers !== null;
  const cardShellDoc = useMemo(
    () =>
      shell === null || !cardUiOpen || chatRef.current === null || !layersReady
        ? null
        : buildShellDocument(shell, readCardStorage(worldId), {
            chat: chatRef.current,
            token: cardShellKey,
            mvu: mvuRef.current,
          }),
    [shell, cardShellKey, worldId, cardUiOpen, layersReady],
  );

  // 每次 render 換上最新的送出函式：訊息監聽只掛一次，不能讓它抓著開面板當下的舊狀態
  const submitTextRef = useRef((_text: string) => Promise.resolve());
  submitTextRef.current = submitText;

  // 卡片寫入的宿主佇列（計畫 8.5）：一份殼一支佇列，換殼、切桌、關面板就關掉（未結算的一律 reject）。
  // 逐字稿與這桌現況用 ref 讀最新值；確認落檔的那則先換進 ref，下一筆寫入才拿得到新版本
  const eventsRef = useRef(events);
  eventsRef.current = events;
  const characterIdRef = useRef(characterId);
  characterIdRef.current = characterId;
  const worldIdRef = useRef(worldId);
  worldIdRef.current = worldId;
  const varsStateRef = useRef(varsState);
  varsStateRef.current = varsState;
  const onEventUpdatedRef = useRef(input.onEventUpdated);
  onEventUpdatedRef.current = input.onEventUpdated;
  const onCardWriteRef = useRef(input.onCardWrite);
  onCardWriteRef.current = input.onCardWrite;
  const writeQueue = useRef<ShellHolder | null>(null);
  // 寫入目標（事件 id 或舊事件的 "@位置"）那一則換成新版：先改 ref（下一筆寫入立刻拿得到），再交給逐字稿
  const replaceHostEvent = useCallback((key: string, update: (previous: TranscriptEvent) => TranscriptEvent) => {
    const list = eventsRef.current;
    const previous = eventOfKey(list, key);
    if (previous === undefined) return;
    const next = update(previous);
    eventsRef.current = list.map((event) => (event === previous ? next : event));
    onEventUpdatedRef.current?.(previous, next);
  }, []);
  useEffect(() => {
    if (!cardUiOpen || shell === null || !worldId) return;
    // 結果所屬身分（送出時快照的桌世代與幕）仍是前端現在這桌的身分
    const sameIdentity = (identity: { generation: number; scene: number }) =>
      worldIdRef.current === worldId &&
      varsStateRef.current?.generation === identity.generation &&
      varsStateRef.current?.scene === identity.scene;
    const holder: ShellHolder = {
      token: cardShellKey,
      reply: null,
      // parseMessage 的值解析 Worker（8.9）：一份殼一支，第一次有請求才真的建
      evals: createEvalHost(),
      queue: createMvuWriteQueue({
        revOf: (key) => {
          if (parseLayerKey(key) !== null) return layersRef.current?.[key]?.rev ?? null;
          const event = eventOfKey(eventsRef.current, key);
          return event === undefined ? undefined : (event.vars_rev ?? null);
        },
        tableOf: (key) =>
          parseLayerKey(key) !== null
            ? (layersRef.current?.[key]?.vars ?? null)
            : (eventOfKey(eventsRef.current, key)?.message_vars ?? null),
        // 世代與幕用產生那張表的快照帶來的，不冒用宿主之後的新值
        send: async (key, expectedRev, payload, identity) => {
          const layerKey = parseLayerKey(key);
          // character 層只認目前殼所屬的卡（沙盒帶的身分以宿主這份為準）
          if (layerKey?.layer === "character" && layerKey.id !== characterIdRef.current) {
            return { status: "rejected", code: "bad-target", found: false, rev: null, table: null };
          }
          // 讀取失敗（或還沒讀回）的層不能寫：未知不等於不存在
          if (layerKey !== null && layerBlocked(layersRef.current, key)) {
            return { status: "rejected", code: "layer-unavailable", found: false, rev: null, table: null };
          }
          const event = eventOfKey(eventsRef.current, key);
          const target = key.startsWith("@") ? { index: Number(key.slice(1)), legacy: event ?? null } : { id: key };
          const result = await (layerKey === null
            ? invoke<CardWriteResult>("card_vars_write", {
                worldId,
                generation: identity.generation,
                scene: identity.scene,
                target,
                expectedRev,
                varsJson: payload,
              })
            : invoke<CardWriteResult>("card_layer_write", {
                worldId,
                layer: layerKey.layer,
                id: layerKey.id,
                generation: identity.generation,
                expectedRev,
                varsJson: payload,
              }));
          // await 回來先核對結果所屬身分：桌換了、或後端的桌世代／幕已不是送出時那組（整桌還原會帶回同 id
          // 的事件），結果就不屬於現在這桌，不換進逐字稿、不推權威值
          const now =
            worldIdRef.current === worldId
              ? await invoke<VarsState>("card_vars_state", { worldId }).catch(() => null)
              : null;
          // 核對本身也要等：等待期間切了桌或前端已換到新世代／幕，同樣不算這桌的結果
          if (
            now === null ||
            now.generation !== identity.generation ||
            now.scene !== identity.scene ||
            !sameIdentity(identity)
          ) {
            if (worldIdRef.current === worldId) void refreshVarsState();
            return { status: "gone" };
          }
          // 第一次卡寫會啟用變數模式；世代或場不符（stale）也重問一次（非 message 層不影響模式）
          if (layerKey === null && (result.status !== "ok" || varsStateRef.current?.active !== true)) void refreshVarsState();
          return result;
        },
        // 換進逐字稿前最後再核一次身分（同步，中間不會再換桌）：佇列關掉後晚到的結果只有身分沒變才換
        committed: (key, next, identity) => {
          if (!sameIdentity(identity)) return;
          replaceHostEvent(key, () => next);
          // 卡片寫入改了有效狀態（後端同步重建快取樹）：主頁狀態欄跟著換新值
          onCardWriteRef.current?.();
        },
        layerCommitted: (key, rev, vars, identity) => {
          if (sameIdentity(identity)) setLayer(key, { rev, vars: JSON.parse(vars) as Record<string, unknown> });
        },
        settle: (results: SettleResult[], authority?: Authority, migrate?: Migration) => {
          // 被拒時後端一併給的權威表與版本也換進宿主現況（逐字稿或非 message 層），下一筆寫入才不會一直拿舊版本被拒
          // 沒有權威表（讀不到、未知）時不動現況：unavailable 狀態只有成功重讀或取得有效權威表才解除
          if (authority && parseLayerKey(authority.key) !== null) {
            if (authority.table !== null) setLayer(authority.key, { rev: authority.rev, vars: authority.table });
          } else if (authority && authority.rev !== (eventOfKey(eventsRef.current, authority.key)?.vars_rev ?? null)) {
            replaceHostEvent(authority.key, (previous) => {
              const next: TranscriptEvent = { ...previous };
              if (authority.table === null) delete next.message_vars;
              else next.message_vars = authority.table;
              if (authority.rev === null) delete next.vars_rev;
              else next.vars_rev = authority.rev;
              return next;
            });
          }
          holder.reply?.postMessage(
            { source: "table-tavern-host", kind: "mvu-settle", token: holder.token, results, authority, migrate },
            { targetOrigin: "*" },
          );
        },
      }),
    };
    writeQueue.current = holder;
    return () => {
      holder.queue.close();
      holder.evals.dispose();
      if (writeQueue.current === holder) writeQueue.current = null;
    };
  }, [cardUiOpen, shell, worldId, cardShellKey, refreshVarsState, replaceHostEvent, setLayer]);

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
      // 卡片寫入變數：token 要是目前這份殼的，形狀不對一律不理
      if (data.kind === "mvu-write") {
        const holder = writeQueue.current;
        if (
          holder === null ||
          data.token !== holder.token ||
          typeof data.requestId !== "string" ||
          typeof data.target !== "string" ||
          typeof data.payload !== "string" ||
          !(typeof data.base === "string" || data.base === null) ||
          !Number.isInteger(data.generation) ||
          !Number.isInteger(data.scene)
        ) {
          return;
        }
        holder.reply = event.source;
        holder.queue.enqueue({
          requestId: data.requestId,
          target: data.target,
          payload: data.payload,
          base: data.base,
          generation: data.generation,
          scene: data.scene,
        });
        return;
      }
      // parseMessage 的值解析與數學式：交給宿主專用 Worker（單筆 200 ms 逾時），結果回給送來的那份沙盒
      if (data.kind === "mvu-eval") {
        const holder = writeQueue.current;
        const request = holder !== null && data.token === holder.token ? parseEvalRequest(data) : null;
        if (holder === null || request === null) return;
        const source = event.source;
        const reply = (outcome: unknown) => {
          if (writeQueue.current !== holder) return;
          source?.postMessage(
            { source: "table-tavern-host", kind: "mvu-eval-result", token: holder.token, requestId: request.requestId, ...(outcome as object) },
            { targetOrigin: "*" },
          );
        };
        // 任何例外（Worker 建不出來、結果送不回）都回錯給沙盒，不讓請求懸著
        void holder.evals
          .run(request.op, request.text)
          .catch((reason: unknown) => ({ ok: false, error: `host-error: ${String(reason)}` }))
          .then(reply);
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
