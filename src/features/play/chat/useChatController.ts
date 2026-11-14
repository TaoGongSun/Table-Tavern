// 聊天 controller：這一幕的逐字稿、收回堆疊、生成中狀態與輸入框，以及玩家送出、
// 角色接話、GM 旁白與推進接力的入口。接力規則與落檔在後端（chat_relay），這裡顯示串流、換進收據、收尾。
import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AppConfig,
  PlayerAppend,
  type RelayEntry,
  type RelayMessage,
  type RelayReport,
  TranscriptEvent,
} from "../../../shared/contracts/backend-contracts";
import { CharacterMeta } from "../../characters/card-model";
import { backendCode } from "../../../shared/ui/backend-text";
import { diceError } from "../../dice/dice-ui";
import { t } from "../../../i18n";
import { showWorldInfoNotices } from "../world-info-notices";
import { askWorldInfoReset, offersWorldInfoReset } from "../world-info-reset";
import { type ChatNotice, useSmartFreeNotices } from "./useSmartFreeNotices";
import {
  consumeFrom,
  pushUndone,
  restorableIndex,
  restorePayload,
  tailMatches,
  type UndoStack,
} from "./undo-stack";
import { useTranscriptStore } from "./useTranscriptStore";
import { lastStoryId, type Landed, type NominatedCall } from "../next-speaker-call";

// GM 點到玩家時後端回這個代號（transport.rs 的 PLAYER_SENTINEL），收到就把發言權交回給玩家
const PLAYER_SENTINEL = "__PLAYER__";

function nowTs() {
  return new Date().toISOString();
}

/** 一輪沒完成：raw 是錯誤原文；draft 是打字送出卻沒能自動收回時，玩家剛送出的原文 */
export interface TurnFailure {
  raw: string;
  draft?: string;
}

export interface ChatController {
  /** 這一幕已落檔的逐字稿 */
  events: TranscriptEvent[];
  /** 免費模型換模提示：只含本次檢視期間自己送出的 turn 的事件，換幕、離桌即清空 */
  notices: ChatNotice[];
  /** 是誰在生成、以哪種形式；id 空字串＝GM */
  generating: { id: string; kind: "dialogue" | "narration" } | null;
  /** 生成中或打字送出整輪進行中：桌次操作、按鈕與輸入框的忙碌判斷都讀這個 */
  busy: boolean;
  /** 同步版的忙碌判斷（ref）：離桌這類 await 之後才檢查的守門用它，不吃舊閉包 */
  isBusy: () => boolean;
  /** 串流到一半的文字 */
  streamText: string;
  input: string;
  setInput: (value: string) => void;
  /** 收回過、且還停在同一桌同一幕，才給復原 */
  canRestore: boolean;
  /** 換桌：整份換掉這一幕的逐字稿 */
  hydrate: (transcript: TranscriptEvent[]) => void;
  /** 卡片寫入落檔後換掉那一則（同一物件或同 id）：只換變數表與版本，不動其他則 */
  replaceEvent: (previous: TranscriptEvent, next: TranscriptEvent) => void;
  /** 檯面被外部改動（復原匯入收掉開場白）後重讀這一幕 */
  reload: () => Promise<void>;
  /** 貼出開場白；true＝真的貼上檯面了，呼叫端據此收掉開場白面板（失敗時面板留著） */
  /** index＝開場白清單序號、importSource＝跳出面板的那次匯入的原檔識別，序號記在那筆匯入的收據上；
   *  isCurrent＝呼叫端的「這次操作還算數嗎」，回來時已換桌就不把事件加進畫面、不刷新；
   *  translated＝text 是翻譯版（後端正文用它，巨集副作用照原檔原文）。後端取得到原檔時正文以原文重新求值 */
  postOpening: (
    text: string,
    index?: number,
    importSource?: string | null,
    isCurrent?: () => boolean,
    translated?: boolean,
  ) => Promise<boolean>;
  undoLast: () => Promise<void>;
  restoreUndone: () => Promise<void>;
  send: (event: FormEvent<HTMLFormElement>) => Promise<void>;
  submitText: (raw: string) => Promise<void>;
  rollDice: (formula: string) => Promise<void>;
  /** 送出並讓點名角色接話：GM 旁白後被點名的角色接一句就停 */
  sendRelay: () => Promise<void>;
  gmNarrate: () => Promise<void>;
  gmAdvance: () => Promise<void>;
  /** 上一則單次旁白點了誰（「讓 X 接話」鈕的來源；鈕出不出現另由 next-speaker-call 推導） */
  nominated: NominatedCall | null;
  /** 讓點名的角色接話（不重跑 GM） */
  callNominated: () => Promise<void>;
  /** 請目前的發言對象接話 */
  replyFromTarget: () => Promise<void>;
  /** 這輪是對話、旁白或換幕整理，送出鍵要換成停止。 */
  canStop: boolean;
  /** 唯一的中止入口。記下目前 turn_id 再請後端只打那一輪。 */
  stopResponse: () => void;
  /** 換幕這類 App 自己跑的長工作：期間畫面顯示 GM 正在生成；回傳這次的 turn id，停止鈕據此中止 */
  beginNarration: () => string;
  endNarration: () => void;
}

// 參數在簽名上直接解構：這支 controller 自己有個叫 input 的 state，
// 沿用其他 controller 的 `input: {...}` 參數名會撞名
export function useChatController({
  worldId,
  scene,
  config,
  speaker,
  gmTargeted,
  vnTable,
  metaOf,
  playerName,
  castCount,
  onArrived,
  reloadAppearances,
  refreshState,
  refreshWorlds,
  noteChatStarted,
  markCliConnected,
  onError,
  onTurnFailed,
}: {
  worldId: string;
  scene: number;
  config: AppConfig | null;
  /** 目前的發言對象（角色 id；GM 時是 App 的 GM 代號） */
  speaker: string;
  /** 發言對象是不是 GM */
  gmTargeted: boolean;
  /** VN 桌：送出後一律走 GM 推進迴圈（GM 出選項或章末、結局就停，後端回 next＝null） */
  vnTable: boolean;
  metaOf: (id: string) => CharacterMeta | undefined;
  /** 玩家卡的名字；沒有玩家卡時 undefined，落到通用稱呼 */
  playerName: string | undefined;
  /** 主區角色數：一個都沒有就沒得接力 */
  castCount: number;
  onArrived: (ids: string[]) => void;
  /** 收回／復原後重算本幕出場集合：登場塊跟著收回或放回，左欄隱藏區同步 */
  reloadAppearances: () => Promise<void>;
  refreshState: () => Promise<void>;
  refreshWorlds: () => Promise<void>;
  noteChatStarted: () => void;
  markCliConnected: () => Promise<void>;
  onError: (message: string) => void;
  /** 回合（送出、請角色發言、GM 旁白、GM 推進）沒完成：交給攔截式彈窗 */
  onTurnFailed: (failure: TurnFailure) => void;
}): ChatController {
  // 這一輪收回的那幾樓，後收的疊在最上面（復原一次放回一樓，順序自然還原）
  const [undone, setUndone] = useState<UndoStack | null>(null);
  const [input, setInputState] = useState("");
  const inputLive = useRef(input);
  const setInput = useCallback((value: string) => { inputLive.current = value; setInputState(value); }, []);
  // 打字送出整輪（含玩家句落檔、失敗收回）期間鎖住輸入框：失敗時框內不可能有新字，原文才能直接放回
  const [sending, setSending] = useState(false);
  // 換桌／換幕世代：await 回來時世代不同（含換走又換回）就不再碰畫面與輸入框
  // 本次檢視期間（這一桌這一幕）由這個畫面送出的 turn id：免費模型換模事件只認這些，
  // 換幕、離桌（世代變）清空，晚到或別處發的舊事件因 turn id 不在集合內被丟棄
  const ownTurns = useRef<Map<string, number>>(new Map());
  const generation = useRef(0);
  const generationKey = useRef(`${worldId}\u0000${scene}`);
  if (generationKey.current !== `${worldId}\u0000${scene}`) {
    generationKey.current = `${worldId}\u0000${scene}`;
    generation.current += 1;
    ownTurns.current = new Map();
  }
  // 這一輪玩家句追加回錯：收尾時重讀逐字稿，讓畫面跟磁碟對齊
  const appendFailed = useRef(false);
  // 後端回報上一條接力還有沒落成的部分（RelayReport.owed）：下一筆追加（玩家句、復原）之前後端會先代落，
  // 那一步回來後重讀逐字稿對齊
  const owed = useRef(false);
  // 逐角色打字指示：狀態帶「是誰在生成、以哪種形式」，不做全域單一指示燈（NewPlan §9.2）
  // id 空字串＝GM（narration 一律如此，dialogue 一定帶角色 id）；顯示名經 metaOf(id) 即時查
  const [generating, setGenerating] = useState<{
    id: string;
    kind: "dialogue" | "narration";
  } | null>(null);
  const [streamText, setStreamText] = useState("");
  // generating 是 state，setState 到重繪中間連點還看得到舊的 false。這支 ref 在進函式當下就佔住。
  // 收回／復原也佔它：逐字稿操作（回合、收回、復原）同一時間只跑一個
  const busyRef = useRef(false);
  // 停止鈕中止的 id：接力期間是接力 id（後端整條不再接、在途那段中止），換幕時是那次換幕的 turn id
  const turnIdRef = useRef<string | null>(null);
  // 在跑的接力：換桌換幕時通知後端在途那段落完就不再接
  const relayRef = useRef<{ id: string; worldId: string } | null>(null);
  // 換幕容量關卡的動作收據（scene_budget::gate）：一個玩家動作（送出、請某某發言、旁白、推進）
  // 一個 id，玩家句與其後所有回覆都帶同一個，後端同一動作只算一次容量
  const actionIdRef = useRef<string | null>(null);
  // 逐字稿存取（畫面清單、本地改動追蹤、重讀、換進後端落檔）；世代仍以上面這一份為準
  const clearUndone = useCallback(() => setUndone(null), []);
  const { events, eventsLive, setEvents, beginWrite, hydrate, reload, replaceEvent, showLanded } = useTranscriptStore({
    worldId,
    scene,
    generation,
    onAppended: clearUndone,
    onError,
  });
  const visibleNotices = useSmartFreeNotices(worldId, scene, generation, ownTurns);
  // 單次旁白記下的點名（世代不同即失效）
  const [pendingCall, setPendingCall] = useState<(NominatedCall & { gen: number }) | null>(null);
  const [canStop, setCanStop] = useState(false);

  /** 同步問「有沒有一輪對話或旁白在跑」：state 的 busy 會晚一拍，守門要用這個 */
  const isBusy = useCallback(() => busyRef.current, []);

  const stopResponse = useCallback(() => {
    const turnId = turnIdRef.current;
    if (turnId) void invoke("chat_abort", { worldId, turnId });
  }, [worldId]);

  // 換桌換幕（世代變）或卸載：在跑的接力在途那段照常落完（落原桌），之後不再發新呼叫
  useEffect(
    () => () => {
      const running = relayRef.current;
      if (running) void invoke("chat_relay_release", { worldId: running.worldId, relayId: running.id });
    },
    [worldId, scene],
  );

  // 房間模式（multiplayer-room 包 2）：GM 鏈在 Rust 執行端跑、直接落檔，落完通知這裡重讀逐字稿
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<{ worldId: string }>("room-table-changed", (event) => {
      if (event.payload.worldId === worldId && !busyRef.current) void reload();
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [worldId, reload]);

  // 收回過、且還停在同一桌同一幕，才給復原（換桌換幕就當這次收回已成定局）。
  // 疊裡全是空白事件（AI 失敗年代留下的那幾則）就不給亮——按了也沒東西回得來。
  // 帶標頭代碼的事件（點名）本文本來就空，也算有內容
  const canRestore = restorableIndex(undone, worldId, scene) >= 0;


  const postOpening = useCallback(
    async (
      text: string,
      index?: number,
      importSource?: string | null,
      isCurrent: () => boolean = () => true,
      translated = false,
    ) => {
      onError("");
      const endWrite = beginWrite();
      try {
        const event = await invoke<TranscriptEvent>("post_opening", {
          worldId,
          scene,
          ts: nowTs(),
          text,
          openingIndex: index ?? null,
          importSource: importSource ?? null,
          translated,
        });
        // 排在回合後面的期間可能已經換桌：開場白照樣落在原桌，但不能加進現在這桌的畫面
        if (!isCurrent()) return true;
        setEvents((previous) => [...previous, event]);
        endWrite();
        setUndone(null);
        await refreshState();
        return true;
      } catch (reason) {
        if (!isCurrent()) return false;
        // 開場白落地撤回時沒還原的變數寫入記在待回報檔；結算不了就給重設出路（重設成了玩家再貼一次）
        await showWorldInfoNotices(worldId);
        if (offersWorldInfoReset(reason)) {
          try {
            if (await askWorldInfoReset(worldId)) return false;
          } catch (resetError) {
            onError(String(resetError));
            return false;
          }
        }
        onError(String(reason));
        return false;
      } finally {
        endWrite();
      }
    },
    [worldId, scene, refreshState, onError, beginWrite],
  );

  // 收回上一句：一次收一樓（登場塊、正文、變動摘要、點名一起走），可連按往回收，收到這一幕見底就停（不動上一幕）
  const undoLast = useCallback(async () => {
    if (busyRef.current || generating !== null || events.length === 0) return;
    busyRef.current = true;
    onError("");
    const started = generation.current;
    const endWrite = beginWrite();
    try {
      const removed = await invoke<TranscriptEvent[]>("pop_transcript_floor", { worldId, scene });
      if (removed.length === 0) return;
      // 後端已刪：先入疊（記在發起時那桌那幕），之後的重讀或換桌都不能讓這樓變成收不回來
      setUndone((previous) => pushUndone(previous, worldId, scene, removed));
      if (generation.current !== started) return;
      if (tailMatches(events, removed)) {
        setEvents((previous) => previous.slice(0, previous.length - removed.length));
        endWrite();
      } else {
        // 畫面落後磁碟（例如後端代落過畫面沒有的那則）：重讀對齊（重讀要等本地改動收尾，先收）
        endWrite();
        await reload();
      }
      await Promise.all([refreshState(), reloadAppearances()]);
    } catch (reason) {
      onError(String(reason));
    } finally {
      endWrite();
      busyRef.current = false;
    }
  }, [generating, events, worldId, scene, refreshState, reloadAppearances, onError, reload, beginWrite]);

  // 復原一次放回一樓（後端一次提交整組寫回），可連按把整輪收回逐樓倒回去。
  // 放回舊句不該把剩下那幾樓一起作廢，只消耗疊頂那一組。
  // 疊頂若是全空白的組就連同丟棄、往下找第一組有內容的放回：空白回合本來就不該存在，
  // 放回去只會讓玩家覺得按鈕壞了（stream-failure-visible）。寫回失敗整組留在疊上，可重按重試
  const restoreUndone = useCallback(async () => {
    const index = restorableIndex(undone, worldId, scene);
    if (!undone || index < 0 || busyRef.current || generating !== null) return;
    busyRef.current = true;
    onError("");
    const started = generation.current;
    const endWrite = beginWrite();
    try {
      // 用後端回傳的那幾則：帶表的事件復原時版本 token 會換新，卡片寫入要拿新版本
      const restored = await invoke<TranscriptEvent[]>("restore_transcript_events", {
        worldId,
        scene,
        events: restorePayload(undone.groups[index]),
      });
      // 後端已寫回：先消耗疊（發起時那桌那幕），之後的重讀失敗或換桌都不能讓這組再被放回一次
      setUndone((previous) => consumeFrom(previous, worldId, scene, index));
      if (generation.current !== started) return;
      if (owed.current) {
        // 後端在放回之前代落了上一輪沒落成的 GM 回覆：重讀對齊（重讀要等本地改動收尾，先收）
        owed.current = false;
        endWrite();
        await reload();
      } else {
        setEvents((previous) => [...previous, ...restored]);
        endWrite();
      }
      await Promise.all([refreshState(), reloadAppearances()]);
    } catch (reason) {
      onError(String(reason));
    } finally {
      endWrite();
      busyRef.current = false;
    }
  }, [undone, generating, worldId, scene, refreshState, reloadAppearances, onError, reload, beginWrite]);

  // 回合沒完成：先把失敗交給彈窗，再（有追加回錯時）重讀逐字稿對齊畫面——重讀可能失敗，
  // 失敗資訊得先落地。世代已變就不重讀，免得蓋掉新桌的畫面（reload 自己也會再核一次）
  const failTurn = useCallback(
    async (reason: unknown, started: number, draft?: string) => {
      onTurnFailed(draft === undefined ? { raw: String(reason) } : { raw: String(reason), draft });
      if (!appendFailed.current || generation.current !== started) return;
      try {
        await reload();
      } catch (error) {
        onError(String(error));
      }
    },
    [onTurnFailed, reload, onError],
  );

  // 收回沒有回覆的玩家句：只有後端回 true（確定已刪）、且世代沒變才算數；回錯一律當沒收
  const discardPlayer = useCallback(
    async (placed: PlayerAppend, text: string, started: number) => {
      try {
        const removed = await invoke<boolean>("discard_unanswered_player", {
          worldId,
          scene,
          offset: placed.offset,
          ts: placed.event.ts,
          text,
        });
        return removed && generation.current === started;
      } catch {
        return false;
      }
    },
    [worldId, scene],
  );

  // 一條接力（不含 busy 防護）：規則與落檔都在後端（chat_relay → chat_turn::relay::drive），這裡只顯示串流、
  // 把後端逐則回報的落檔換進畫面。世代已變就不開新接力；換桌後才到的回報落在原桌、不碰現在的畫面。
  // Channel 的回報可能晚於 invoke 回來：收尾照收據順序補上沒送到的那幾則。
  // onLanded：每則落檔當下呼叫（換桌後才落的也算）。失敗不拋：回傳的 report.error 交呼叫端照收據收尾。
  // stale＝最後一段旁白落檔後還沒刷新檯面（接力中途在下一段開始時刷新，最後一段交 finishRelay）
  const runRelay = useCallback(
    async (
      started: number,
      entry: RelayEntry,
      onLanded?: (landed: Landed) => void,
    ): Promise<(RelayReport & { stale: boolean }) | null> => {
      if (generation.current !== started) return null;
      noteChatStarted();
      const relayId = crypto.randomUUID();
      turnIdRef.current = relayId;
      relayRef.current = { id: relayId, worldId };
      setCanStop(true);
      let open = true;
      let seen = 0;
      let stale = false;
      const land = (event: TranscriptEvent, next: string | null) => {
        seen += 1;
        if (event.kind === "narration") stale = true;
        showLanded(event, started);
        onLanded?.({ id: generation.current === started ? event.id : undefined, next });
      };
      const onEvent = new Channel<RelayMessage>();
      onEvent.onmessage = (message) => {
        const current = generation.current === started;
        if (message.type === "start") {
          if (!open || !current) return;
          // 錨點＝這段開始當下的逐字稿長度（玩家句已在其中）：這段的換模提示固定插在這裡，也就是回覆之前
          ownTurns.current.set(message.turnId, eventsLive.current.length);
          setGenerating({ id: message.speaker, kind: message.kind });
          setStreamText("");
          // 上一段旁白已提交狀態：接力中途就刷新檯面，刷不成只提示
          if (stale) {
            stale = false;
            refreshState().catch((reason: unknown) => onError(String(reason)));
          }
        } else if (message.type === "delta") {
          if (open && current) setStreamText((previous) => previous + message.text);
        } else if (message.type === "arrived") {
          // 這輪劇情帶出場的卡：併入本幕出場集合，auto_hidden 卡立刻從隱藏區移回主區
          if (current && message.ids.length > 0) onArrived(message.ids);
        } else if (open) {
          land(message.event, message.next);
        }
      };
      let report: RelayReport;
      try {
        report = await invoke<RelayReport>("chat_relay", {
          worldId,
          relayId,
          mode: entry.kind,
          cardId: "cardId" in entry ? entry.cardId : null,
          narrationId: "narrationId" in entry ? entry.narrationId : null,
          actionId: actionIdRef.current,
          // 玩家沒名字就存空字串，顯示與送 AI 時再照當下語系補稱呼
          playerName: playerName ?? "",
          onEvent,
        });
      } finally {
        open = false;
        relayRef.current = null;
        void showWorldInfoNotices(worldId);
      }
      for (const event of report.receipts.slice(seen)) land(event, null);
      owed.current = report.owed;
      // 開場時後端代落了上一回合沒落成的 GM 回覆：重讀，畫面順序跟磁碟一致
      if (report.resync && generation.current === started) await reload();
      // 狀態更新沒寫成：正文照落，提示這一輪的狀態可能沒更新
      for (const message of report.stateErrors) onError(message);
      return { ...report, stale };
    },
    [noteChatStarted, worldId, playerName, showLanded, eventsLive, onArrived, refreshState, reload, onError],
  );

  // 接力收尾：最後一段旁白落檔後還沒刷新就刷新檯面（世代沒變才刷），失敗就拋（收據已由 runRelay 換進畫面）；
  // 有任何落檔才記 CLI 連得上
  const finishRelay = useCallback(
    async (started: number, report: (RelayReport & { stale: boolean }) | null) => {
      if (!report) return;
      if (report.stale && generation.current === started) {
        try {
          await refreshState();
        } catch (reason) {
          if (report.error === null) throw reason;
          onError(String(reason));
        }
      }
      if (report.error !== null) throw report.error;
      if (report.receipts.length > 0) await markCliConnected();
    },
    [refreshState, markCliConnected, onError],
  );

  const endTurn = useCallback(() => {
    busyRef.current = false;
    actionIdRef.current = null;
    turnIdRef.current = null;
    setCanStop(false);
    setGenerating(null);
    setStreamText("");
  }, []);

  // 按鈕發起的一條接力：旁白、推進、點名、請 X 發言共用（失敗交彈窗，不收任何玩家句）
  const runButton = useCallback(
    async (entry: RelayEntry, onLanded?: (started: number, landed: Landed) => void) => {
      busyRef.current = true;
      actionIdRef.current = crypto.randomUUID();
      appendFailed.current = false;
      const started = generation.current;
      onError("");
      try {
        await finishRelay(started, await runRelay(started, entry, onLanded && ((landed) => onLanded(started, landed))));
        await refreshWorlds();
      } catch (reason) {
        await failTurn(reason, started);
      } finally {
        endTurn();
      }
    },
    [runRelay, finishRelay, refreshWorlds, onError, failTurn, endTurn],
  );

  // 點名指定角色接話；也是「請 X 發言」按鈕的入口（NewPlan §9、MVP 第 8 項）：不寫點名列
  const requestReply = useCallback(
    async (characterId: string) => {
      if (!characterId || busyRef.current) return;
      // 讀不到或已切成舞台卡：後端不會接話，提示一句而不是靜默
      const meta = metaOf(characterId);
      if (!meta || meta.stage) {
        onError(t("requestReplyUnavailable", { name: meta?.name ?? t("characterFallback") }));
        return;
      }
      await runButton({ kind: "request", cardId: characterId });
    },
    [metaOf, onError, runButton],
  );

  // 單次旁白的點名記下來給「讓 X 接話」鈕：只記點到角色、世代沒變的；鈕何時出現由 next-speaker-call 推導
  const rememberCall = useCallback((started: number, landed: Landed) => {
    if (!landed.id || !landed.next || landed.next === PLAYER_SENTINEL || generation.current !== started) return;
    setPendingCall({ narrationId: landed.id, characterId: landed.next, gen: started });
  }, []);

  // 簡易導演：GM 插入旁白（NewPlan §6.1、MVP 第 9 項）；點名不自動接，記下來讓玩家按「讓 X 接話」
  const gmNarrate = useCallback(async () => {
    if (busyRef.current) return;
    await runButton({ kind: "narrate" }, rememberCall);
  }, [runButton, rememberCall]);

  // 「讓 X 接話」：接上一則旁白記下的點名，不重跑 GM、不改發言對象；最後一則故事已不是那則旁白就不接。
  // 後端寫「GM 請 X 發言」列（該旁白之後已有同名的就不疊）再接話；點名後才被切成舞台卡就不寫列、不接
  const callNominated = useCallback(async () => {
    const call = pendingCall;
    if (!call || busyRef.current || call.gen !== generation.current) return;
    if (lastStoryId(eventsLive.current) !== call.narrationId) return;
    if (metaOf(call.characterId)?.stage === true) return;
    await runButton({ kind: "nominated", cardId: call.characterId, narrationId: call.narrationId });
  }, [pendingCall, eventsLive, metaOf, runButton]);

  // 簡易導演：「GM 推進」鈕。旁白＋點名→角色接話的接力，至「輪到玩家」、GM 沒點名（含 VN 停點）
  // 或每回合上限停下（NewPlan §6.1）
  const gmAdvance = useCallback(async () => {
    if (!config || busyRef.current || castCount === 0) return;
    await runButton({ kind: "advance" });
  }, [config, castCount, runButton]);

  // 請目前的發言對象接話：GM 以旁白回應（讀得到世界設定與全部角色卡），角色就點名接話
  const replyFromTarget = useCallback(async () => {
    if (busyRef.current) return;
    if (gmTargeted) await gmNarrate();
    else if (speaker) await requestReply(speaker);
  }, [gmTargeted, speaker, gmNarrate, requestReply]);

  // 打字送出（fromComposer）與卡片介面送出共用。只有打字送出會清輸入框、鎖輸入框，
  // 失敗時也只有它走「乾淨路徑」自動收回：玩家句落檔並拿到收據、之後沒回成，後端核對那一行
  // 確實還是檔尾才截掉，確定刪了才把原文放回輸入框；其他任何情況都不收、不放回，
  // 原文改放在失敗彈窗裡（見 .ai/plans/quota-insufficient-alert.md）。
  // GM 正文或角色對白一落檔（以後端收據為準）玩家句就不是檔尾：之後任何一步失敗都保留玩家句、彈窗不帶原文（免得誘導重送）。
  // relay＝「送出並讓點名角色接話」：旁白點到角色就接一句就停，點到玩家只寫點名列
  const submitTurn = useCallback(
    async (raw: string, fromComposer: boolean, relay = false) => {
      const text = raw.trim();
      if (busyRef.current || (relay && !text)) return;
      // 卡片只按了 /trigger（沒帶文字）＝直接要對象接話，不留玩家發言
      if (!text) {
        if (gmTargeted) await gmNarrate();
        else if (speaker) await requestReply(speaker);
        return;
      }
      busyRef.current = true;
      actionIdRef.current = crypto.randomUUID();
      appendFailed.current = false;
      const started = generation.current;
      onError("");
      let replied = false;
      if (fromComposer) {
        setSending(true);
        setInput("");
      }
      let placed: PlayerAppend | null = null;
      let endWrite = beginWrite();
      try {
        try {
          placed = await invoke<PlayerAppend>("append_player_event", {
            worldId,
            scene,
            event: { ts: nowTs(), speaker_id: "", speaker_name: playerName ?? "", kind: "player", text, action_id: actionIdRef.current ?? undefined },
            actionId: actionIdRef.current,
          });
        } catch (reason) {
          appendFailed.current = true;
          throw reason;
        }
        if (generation.current === started) {
          const shown = placed.event;
          if (owed.current) {
            // 上一輪沒落成的 GM 回覆已由後端在這句之前代落：重讀，畫面順序跟磁碟一致
            owed.current = false;
            endWrite();
            try {
              await reload();
            } catch {
              setEvents((previous) => [...previous, shown]);
            }
          } else {
            setEvents((previous) => [...previous, shown]);
          }
          // 桌上一有新內容，收回的那幾句就不能再放回去了
          setUndone(null);
        }
        endWrite();
        // 玩家句落檔時已經換桌：句子留在原桌，不再替原桌發 AI 呼叫
        if (generation.current !== started) return;
        // VN 桌：玩家句之後走推進迴圈，不看發言對象
        const entry: RelayEntry | null = vnTable
          ? { kind: "advance" }
          : gmTargeted
            ? { kind: relay ? "narrateThenOne" : "narrate" }
            : speaker
              ? { kind: "request", cardId: speaker }
              : null;
        if (entry) {
          const remember = gmTargeted && !vnTable ? (landed: Landed) => rememberCall(started, landed) : undefined;
          const report = await runRelay(started, entry, remember);
          replied = report !== null && report.receipts.some((event) => event.kind !== "system");
          await finishRelay(started, report);
        }
        await refreshWorlds();
      } catch (reason) {
        endWrite();
        if (!fromComposer || replied) {
          await failTurn(reason, started);
          return;
        }
        // 換幕容量關卡在落玩家句之前就擋下：句子沒落檔，原文放回輸入框，換完幕還送得出去
        if (!placed && backendCode(reason) === "scene_capacity_full") {
          if (generation.current === started) setInput(raw);
          await failTurn(reason, started);
          return;
        }
        let discarded = false;
        if (placed && generation.current === started) {
          const shown = placed.event;
          endWrite = beginWrite();
          discarded = await discardPlayer(placed, text, started);
          // 重讀過的畫面裡是另一個物件：同 id 也算這句
          if (discarded) {
            setEvents((previous) =>
              previous.filter((event) => event !== shown && (shown.id === undefined || event.id !== shown.id)),
            );
          }
          endWrite();
        }
        if (discarded) {
          setInput(raw);
          await failTurn(reason, started);
          try {
            await refreshState();
          } catch (error) {
            onError(String(error));
          }
        } else {
          await failTurn(reason, started, raw);
        }
      } finally {
        endWrite();
        endTurn();
        if (fromComposer) setSending(false);
      }
    },
    [gmTargeted, vnTable, speaker, gmNarrate, requestReply, worldId, scene, playerName, onError, runRelay, finishRelay, rememberCall, refreshWorlds, failTurn, discardPlayer, refreshState, reload, beginWrite, setEvents, endTurn],
  );

  const rollDice = useCallback(async (formula: string) => {
    if (busyRef.current) return;
    busyRef.current = true;
    setSending(true);
    const started = generation.current;
    const wasEmpty = !inputLive.current.trim();
    const endWrite = beginWrite();
    let saved = false;
    onError("");
    try {
      const result = await invoke<TranscriptEvent>("roll_dice", { worldId, scene, formula });
      showLanded(result, started);
      if (generation.current === started) owed.current = false;
      saved = true;
    } catch (reason) {
      if (generation.current === started) onError(diceError(reason));
    } finally {
      endWrite();
      if (generation.current === started) {
        // 失敗也只重讀，絕不補擲；成功時也收進被代落的前輪正文。
        try { await reload(); } catch (reason) { saved = false; if (generation.current === started) onError(String(reason)); }
      }
      busyRef.current = false;
      setSending(false);
    }
    if (saved && generation.current === started && wasEmpty && !inputLive.current.trim()) {
      await submitTurn("", false);
    }
  }, [worldId, scene, beginWrite, reload, showLanded, onError, submitTurn]);

  const submitText = useCallback((raw: string) => submitTurn(raw, false), [submitTurn]);
  const sendRelay = useCallback(() => submitTurn(input, true, true), [submitTurn, input]);

  const send = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      await submitTurn(input, true);
    },
    [submitTurn, input],
  );

  // 換幕／重生摘要跑在 App 那頭，但畫面上那段「GM 正在生成」屬於這裡
  const beginNarration = useCallback(() => {
    busyRef.current = true;
    const turnId = crypto.randomUUID();
    turnIdRef.current = turnId;
    setCanStop(true);
    setGenerating({ id: "", kind: "narration" });
    setStreamText("");
    return turnId;
  }, []);

  const endNarration = useCallback(() => {
    busyRef.current = false;
    turnIdRef.current = null;
    setCanStop(false);
    setGenerating(null);
    setStreamText("");
  }, []);

  return useMemo(
    () => ({
      events,
      notices: visibleNotices,
      generating,
      busy: generating !== null || sending,
      isBusy,
      streamText,
      input,
      setInput,
      canRestore,
      hydrate,
      replaceEvent,
      reload,
      postOpening,
      undoLast,
      restoreUndone,
      send,
      submitText,
      rollDice,
      sendRelay,
      gmNarrate,
      gmAdvance,
      replyFromTarget,
      nominated: pendingCall && pendingCall.gen === generation.current
        ? { narrationId: pendingCall.narrationId, characterId: pendingCall.characterId }
        : null,
      callNominated,
      canStop,
      stopResponse,
      beginNarration,
      endNarration,
    }),
    [
      events,
      visibleNotices,
      generating,
      sending,
      isBusy,
      streamText,
      input,
      canRestore,
      hydrate,
      replaceEvent,
      reload,
      postOpening,
      undoLast,
      restoreUndone,
      send,
      submitText,
      rollDice,
      sendRelay,
      gmNarrate,
      gmAdvance,
      replyFromTarget,
      pendingCall,
      callNominated,
      canStop,
      stopResponse,
      beginNarration,
      endNarration,
    ],
  );
}
