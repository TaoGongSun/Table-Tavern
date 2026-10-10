// 聊天 controller：這一幕的逐字稿、收回堆疊、生成中狀態與輸入框，以及玩家送出、
// 角色接話、GM 旁白與推進接力的整條流程。所有權從 App() 搬過來，行為與依賴陣列照舊。
import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  createEventDeduper,
  FAILOVER_EVENT,
  type FailoverPayload,
  failoverNotice,
  SWITCHED_EVENT,
  type SmartFreeNotice,
  type SwitchedPayload,
  switchedNotice,
} from "../ai-connection/smart-free-events";
import { AppConfig, PlayerAppend, TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { CharacterMeta } from "../characters/card-model";
import { parseMarker } from "../../shared/ui/event-text";
import { backendCode } from "../../shared/ui/backend-text";
import { showWorldInfoNotices } from "./world-info-notices";
import { askWorldInfoReset, offersWorldInfoReset } from "./world-info-reset";

/** 有本文，或帶已知標頭代碼（點名事件本文本來就空）才算有內容；空白回合不落地、不放回 */
function hasContent(event: TranscriptEvent): boolean {
  return event.text.trim() !== "" || parseMarker(event.marker) !== null;
}

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

/** 一個換桌／換幕世代的本地改動追蹤：rev 每次改動換進畫面加一、inFlight 在途數、waiters 等在途清空的重讀 */
interface WriteTrack {
  generation: number;
  rev: number;
  inFlight: number;
  waiters: (() => void)[];
}

/** 聊天室裡的一行非持久系統提示（不進逐字稿）；at＝出現時逐字稿有幾則，畫面據此插在那個位置 */
export interface ChatNotice {
  id: string;
  at: number;
  notice: SmartFreeNotice;
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
  gmNarrate: () => Promise<void>;
  gmAdvance: () => Promise<void>;
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
  metaOf,
  playerName,
  castCount,
  onArrived,
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
  metaOf: (id: string) => CharacterMeta | undefined;
  /** 玩家卡的名字；沒有玩家卡時 undefined，落到通用稱呼 */
  playerName: string | undefined;
  /** 主區角色數：一個都沒有就沒得接力 */
  castCount: number;
  onArrived: (ids: string[]) => void;
  refreshState: () => Promise<void>;
  refreshWorlds: () => Promise<void>;
  noteChatStarted: () => void;
  markCliConnected: () => Promise<void>;
  onError: (message: string) => void;
  /** 回合（送出、請角色發言、GM 旁白、GM 推進）沒完成：交給攔截式彈窗 */
  onTurnFailed: (failure: TurnFailure) => void;
}): ChatController {
  const [events, setEventsState] = useState<TranscriptEvent[]>([]);
  // 逐字稿的同步副本：每次更新先算進這裡再交給 React，讓事件／回合開始時讀得到「已 setEvents 但還沒渲染」的長度
  const eventsLive = useRef<TranscriptEvent[]>([]);
  const setEvents = useCallback(
    (next: TranscriptEvent[] | ((previous: TranscriptEvent[]) => TranscriptEvent[])) => {
      eventsLive.current = typeof next === "function" ? next(eventsLive.current) : next;
      setEventsState(eventsLive.current);
    },
    [],
  );
  // 這一輪收回的那幾句，後收的疊在最上面（復原一次拿一則，順序自然還原）。
  // 記下當時的桌與幕，換桌換幕後整疊自動失效（比對不上就不顯示），免得放回錯的地方
  const [undone, setUndone] = useState<{
    worldId: string;
    scene: number;
    events: TranscriptEvent[];
  } | null>(null);
  const [input, setInput] = useState("");
  // 打字送出整輪（含玩家句落檔、失敗收回）期間鎖住輸入框：失敗時框內不可能有新字，原文才能直接放回
  const [sending, setSending] = useState(false);
  // 換桌／換幕世代：await 回來時世代不同（含換走又換回）就不再碰畫面與輸入框
  // 本次檢視期間（這一桌這一幕）由這個畫面送出的 turn id：免費模型換模事件只認這些，
  // 換幕、離桌（世代變）清空，晚到或別處發的舊事件因 turn id 不在集合內被丟棄
  const ownTurns = useRef<Map<string, number>>(new Map());
  const generation = useRef(0);
  // 本地改動追蹤（見 beginWrite／reload）按世代各一份：換世代換新的一份並叫醒舊世代的等待者，
  // 舊桌永不回應的落檔拖不住新桌的重讀，舊桌晚到的收尾也碰不到新桌的計數
  const writes = useRef<WriteTrack>({ generation: 0, rev: 0, inFlight: 0, waiters: [] });
  const generationKey = useRef(`${worldId}\u0000${scene}`);
  if (generationKey.current !== `${worldId}\u0000${scene}`) {
    generationKey.current = `${worldId}\u0000${scene}`;
    generation.current += 1;
    ownTurns.current = new Map();
    const old = writes.current;
    writes.current = { generation: generation.current, rev: 0, inFlight: 0, waiters: [] };
    for (const wake of old.waiters.splice(0)) wake();
  }
  // 這一輪有追加回錯：收尾時重讀逐字稿，讓畫面跟磁碟對齊
  const appendFailed = useRef(false);
  // GM 正文重試後仍沒落檔：後端在下一筆新事件追加前（玩家句、復原）或下一個回合開始時用留下的正文代落，
  // 那一步回來後重讀逐字稿對齊
  const mainLost = useRef(false);
  // 逐角色打字指示：狀態帶「是誰在生成、以哪種形式」，不做全域單一指示燈（NewPlan §9.2）
  // id 空字串＝GM（narration 一律如此，dialogue 一定帶角色 id）；顯示名經 metaOf(id) 即時查
  const [generating, setGenerating] = useState<{
    id: string;
    kind: "dialogue" | "narration";
  } | null>(null);
  const [streamText, setStreamText] = useState("");
  const responseTruncated = useRef(false);
  // generating 是 state，setState 到重繪中間連點還看得到舊的 false。這支 ref 在進函式當下就佔住。
  // 收回／復原也佔它：逐字稿操作（回合、收回、復原）同一時間只跑一個
  const busyRef = useRef(false);
  const stopRequested = useRef(false);
  const turnIdRef = useRef<string | null>(null);
  // 換幕容量關卡的動作收據（scene_budget::gate）：一個玩家動作（送出、請某某發言、旁白、推進）
  // 一個 id，玩家句與其後所有回覆都帶同一個，後端同一動作只算一次容量
  const actionIdRef = useRef<string | null>(null);
  const [notices, setNotices] = useState<(ChatNotice & { gen: number })[]>([]);
  const [canStop, setCanStop] = useState(false);

  /** 同步問「有沒有一輪對話或旁白在跑」：state 的 busy 會晚一拍，守門要用這個 */
  const isBusy = useCallback(() => busyRef.current, []);

  const beginTurn = () => {
    stopRequested.current = false;
    const turnId = crypto.randomUUID();
    turnIdRef.current = turnId;
    // 錨點＝回合開始當下的逐字稿長度（玩家句已在其中）：這輪的提示固定插在這裡，也就是回覆之前
    ownTurns.current.set(turnId, eventsLive.current.length);
    return turnId;
  };

  const stopResponse = useCallback(() => {
    const turnId = turnIdRef.current;
    stopRequested.current = true;
    if (turnId) void invoke("chat_abort", { worldId, turnId });
  }, [worldId]);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<{ world: string | null; reason: string }>("ai-response-truncated", (event) => {
      if (event.payload.world === worldId) responseTruncated.current = true;
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [worldId]);

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const accept = createEventDeduper();
    const gen = generation.current;
    const show = (
      payload: { eventId: string; world: string | null; turnId: string | null },
      notice: SmartFreeNotice,
    ) => {
      if (disposed || generation.current !== gen) return;
      if (payload.turnId === null || payload.world !== worldId) return;
      const anchor = ownTurns.current.get(payload.turnId);
      if (anchor === undefined) return;
      if (!accept(payload.eventId)) return;
      setNotices((previous) => [
        ...previous,
        { id: payload.eventId, at: anchor, notice, gen },
      ]);
    };
    const track = (promise: Promise<() => void>) =>
      void promise.then((stop) => {
        if (disposed) stop();
        else stops.push(stop);
      });
    track(listen<FailoverPayload>(FAILOVER_EVENT, (event) => show(event.payload, failoverNotice(event.payload))));
    track(listen<SwitchedPayload>(SWITCHED_EVENT, (event) => show(event.payload, switchedNotice(event.payload))));
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
      setNotices([]);
    };
  }, [worldId, scene]);

  const takeResponseTruncated = useCallback(async () => {
    // Rust 端先 emit 再讓 invoke resolve；讓事件佇列多一個 tick 完成投遞。
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    const truncated = responseTruncated.current;
    responseTruncated.current = false;
    return truncated;
  }, []);

  // 收回過、且還停在同一桌同一幕，才給復原（換桌換幕就當這次收回已成定局）。
  // 疊裡全是空白事件（AI 失敗年代留下的那幾則）就不給亮——按了也沒東西回得來。
  // 帶標頭代碼的事件（點名）本文本來就空，也算有內容
  const canRestore =
    undone !== null &&
    undone.worldId === worldId &&
    undone.scene === scene &&
    undone.events.some(hasContent);

  // 畫面上的逐字稿除了重讀，還會被本地改動換掉：先落檔、回來再把結果換進畫面（追加、收回、卡片寫入換表）。
  // 重讀從發出到回來之間只要有本地改動完成或還在途，讀到的那份就可能比畫面舊（蓋回舊表、吃掉剛追加的事件），
  // 或跟函式型追加疊成兩份——那份不套用、等改動落定再讀。rev 在每次本地改動換進畫面時加一
  /** 本地改動開始（落檔前呼叫）；回傳的收尾在結果換進畫面之後呼叫，重複呼叫只算一次，只動發起當時那個世代的計數 */
  const beginWrite = useCallback(() => {
    const track = writes.current;
    track.inFlight += 1;
    let open = true;
    return () => {
      if (!open) return;
      open = false;
      track.inFlight -= 1;
      track.rev += 1;
      if (track.inFlight === 0) for (const wake of track.waiters.splice(0)) wake();
    };
  }, []);

  const hydrate = useCallback((transcript: TranscriptEvent[]) => {
    writes.current.rev += 1;
    setEvents(transcript);
  }, []);

  // 重讀：同時只跑一趟（同世代），在途期間再被要求就在這趟回來後補讀一次；呼叫端拿到的 promise
  // 等最後一趟套用完才 resolve。讀的期間有本地改動（見 beginWrite）就整份作廢重讀，在途改動先等它落定
  const reloading = useRef<{ generation: number; again: boolean; promise: Promise<void> } | null>(null);
  const reload = useCallback(() => {
    const started = generation.current;
    const running = reloading.current;
    if (running && running.generation === started) {
      running.again = true;
      return running.promise;
    }
    const job = { generation: started, again: false, promise: Promise.resolve() };
    job.promise = (async () => {
      try {
        for (;;) {
          job.again = false;
          const track = writes.current;
          if (track.generation !== started) return;
          while (track.inFlight > 0) {
            await new Promise<void>((wake) => track.waiters.push(wake));
            if (generation.current !== started) return;
          }
          const rev = track.rev;
          const transcript = await invoke<TranscriptEvent[]>("read_transcript", { worldId, scene });
          if (generation.current !== started) return;
          if (track.rev !== rev || track.inFlight > 0) continue;
          setEvents(transcript);
          if (!job.again) return;
        }
      } finally {
        if (reloading.current === job) reloading.current = null;
      }
    })();
    reloading.current = job;
    return job.promise;
  }, [worldId, scene]);

  // 卡片寫入的落檔在卡片 controller 的佇列裡，這裡只在確認後拿到結果：換掉同一物件或同 id 那一則（只會有一則）。
  // 第一次被寫入的舊事件原本沒 id，畫面若在這期間重讀過就認不出是哪一則（同時間同正文的舊事件可能不只一則），
  // 不猜：照樣只換同一物件，另外重讀一次拿後端的權威結果
  const replaceEvent = useCallback(
    (previous: TranscriptEvent, next: TranscriptEvent) => {
      writes.current.rev += 1;
      setEvents((list) =>
        list.map((event) =>
          event === previous || (event.id !== undefined && event.id === next.id) ? next : event,
        ),
      );
      if (previous.id === undefined) reload().catch((reason: unknown) => onError(String(reason)));
    },
    [reload, onError],
  );

  // started＝發起這一輪時的世代，由回合一路傳下來：換桌之後才呼叫也不會把新世代當成自己的
  const appendEvent = useCallback(
    // turn＝GM 回合的一部分（main 正文／state_update 變動紀錄）：後端以 (turn_id, turn_part) 冪等落檔，
    // main 掛上回合算好的變數表；characterTurn＝這則是那次角色聊天呼叫的回覆，後端憑它認世界書落地成功
    async (
      event: TranscriptEvent,
      started: number,
      turn?: { turnId: string; part: "main" | "state_update" },
      characterTurn?: string,
    ) => {
      // 空白事件一律不落地（stream-failure-visible）：AI 失敗時故事不該多出一則看不見的
      // 回合，它還會進下一次呼叫的歷史把模型帶偏。後端 API 路徑已擋，這裡是 CLI 路徑
      // 與任何未來新路徑的保險，錯誤碼與後端同一個
      // 帶已知標頭代碼的事件（點名）本文可以是空的
      if (!hasContent(event)) {
        throw new Error("AI_EMPTY_RESPONSE: 空白回合不寫進故事");
      }
      // 用後端回傳的那份（快照已補好）進畫面：收回後要復原時，送回去的事件才帶著當時的
      // 檯面值，狀態欄跟著回到那一刻
      let stamped: TranscriptEvent;
      // 動作中寫下的每一則都蓋上這個動作的 id：後端換幕容量預測靠它切出每個動作的回覆量
      const action = actionIdRef.current;
      const endWrite = beginWrite();
      try {
        try {
          stamped = await invoke<TranscriptEvent>("append_transcript", {
            worldId,
            scene,
            event: action ? { ...event, action_id: action } : event,
            turnId: turn?.turnId ?? null,
            turnPart: turn?.part ?? null,
            characterTurn: characterTurn ?? null,
          });
        } catch (reason) {
          appendFailed.current = true;
          throw reason;
        }
        if (generation.current !== started) return;
        setEvents((previous) => [...previous, stamped]);
      } finally {
        endWrite();
      }
      // 桌上一有新內容，收回的那幾句就不能再放回去了——位置已經被後面的話蓋掉
      setUndone(null);
    },
    [worldId, scene, beginWrite],
  );

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

  // 收回上一句：一次砍一則、可連按往回收，收到這一幕見底就停（不動上一幕）
  const undoLast = useCallback(async () => {
    if (busyRef.current || generating !== null || events.length === 0) return;
    busyRef.current = true;
    onError("");
    const last = events[events.length - 1];
    const started = generation.current;
    const endWrite = beginWrite();
    try {
      if (!(await invoke<boolean>("pop_transcript", { worldId, scene }))) return;
      if (generation.current !== started) return;
      setEvents((previous) => previous.slice(0, -1));
      endWrite();
      setUndone((previous) =>
        previous && previous.worldId === worldId && previous.scene === scene
          ? { ...previous, events: [...previous.events, last] }
          : { worldId, scene, events: [last] },
      );
      await refreshState();
    } catch (reason) {
      onError(String(reason));
    } finally {
      endWrite();
      busyRef.current = false;
    }
  }, [generating, events, worldId, scene, refreshState, onError, beginWrite]);

  // 復原一次放回一則，可連按把整輪收回逐則倒回去。
  // 這裡不走 appendEvent——放回舊句不該把剩下那幾句一起作廢，只消耗疊頂那一則。
  // 疊頂若是空白事件就連同丟棄、往下找第一則有內容的放回：空白回合本來就不該存在，
  // 放回去只會讓玩家覺得按鈕壞了（stream-failure-visible）
  const restoreUndone = useCallback(async () => {
    if (!undone || !canRestore || busyRef.current || generating !== null) return;
    busyRef.current = true;
    let index = undone.events.length - 1;
    while (index >= 0 && !hasContent(undone.events[index])) index -= 1;
    const event = undone.events[index];
    onError("");
    const started = generation.current;
    const endWrite = beginWrite();
    try {
      // 用後端回傳的那份：帶表的事件復原時版本 token 會換新，卡片寫入要拿新版本。表以 JSON 文字送回
      // （Tauri 參數會把物件鍵排序，文字才保得住原順序）
      const payload = event.message_vars ? { ...event, message_vars: JSON.stringify(event.message_vars) } : event;
      const restored = await invoke<TranscriptEvent>("append_transcript", { worldId, scene, event: payload });
      if (generation.current !== started) return;
      if (mainLost.current) {
        // 後端在放回這句之前代落了上一輪沒落成的 GM 回覆：重讀對齊（重讀要等本地改動收尾，先收）
        mainLost.current = false;
        endWrite();
        await reload();
      } else {
        setEvents((previous) => [...previous, restored]);
        endWrite();
      }
      setUndone((previous) =>
        previous && index > 0
          ? { ...previous, events: previous.events.slice(0, index) }
          : null,
      );
      await refreshState();
    } catch (reason) {
      onError(String(reason));
    } finally {
      endWrite();
      busyRef.current = false;
    }
  }, [undone, canRestore, generating, worldId, scene, refreshState, onError, reload, beginWrite]);

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

  // 單次角色接話（不含 busy 防護），供手動點名與 GM 推進共用；失敗往外拋由呼叫端收尾。
  // 世代已變就不開新的 AI 呼叫；已在路上的回覆照樣落進原桌（closure 的 worldId），只是不碰現在的畫面
  const replyOnce = useCallback(
    async (characterId: string, started: number) => {
      if (generation.current !== started) return;
      noteChatStarted();
      const turnId = beginTurn();
      setCanStop(true);
      setGenerating({ id: characterId, kind: "dialogue" });
      setStreamText("");
      responseTruncated.current = false;
      const onDelta = new Channel<string>();
      onDelta.onmessage = (delta) => {
        if (generation.current === started) setStreamText((previous) => previous + delta);
      };
      // raw：剝殼前的台詞原文（卡片介面讀它），與 text 相同時是 null
      const reply = await invoke<{ text: string; raw: string | null; aborted: boolean }>("chat_with_character", {
        worldId,
        characterId,
        turnId,
        onDelta,
        actionId: actionIdRef.current,
      }).finally(() => void showWorldInfoNotices(worldId));
      // 角色回合開始時後端代落了上一輪沒落成的 GM 正文：重讀逐字稿對齊
      if (mainLost.current) {
        mainLost.current = false;
        await reload();
      }
      const name = metaOf(characterId)?.name ?? "";
      if (reply.aborted) {
        await takeResponseTruncated();
        if (reply.text.trim()) {
          await appendEvent({
            ts: nowTs(),
            speaker_id: characterId,
            speaker_name: name,
            kind: "dialogue",
            text: reply.text,
            ...(reply.raw ? { raw: reply.raw } : {}),
            truncated: true,
          }, started, undefined, turnId);
          await markCliConnected();
        }
        return;
      }
      const truncated = await takeResponseTruncated();
      await appendEvent({
        ts: nowTs(),
        speaker_id: characterId,
        speaker_name: name,
        kind: "dialogue",
        text: reply.text,
        ...(reply.raw ? { raw: reply.raw } : {}),
        ...(truncated ? { truncated: true } : {}),
      }, started, undefined, turnId);
      await markCliConnected();
    },
    [noteChatStarted, worldId, metaOf, appendEvent, markCliConnected, takeResponseTruncated, reload],
  );

  // 點名指定角色接話；也是「請 X 發言」按鈕的入口（NewPlan §9、MVP 第 8 項）
  const requestReply = useCallback(
    async (characterId: string) => {
      if (!characterId || busyRef.current) return;
      busyRef.current = true;
      actionIdRef.current = crypto.randomUUID();
      appendFailed.current = false;
      const started = generation.current;
      onError("");
      try {
        await replyOnce(characterId, started);
        await refreshWorlds();
      } catch (reason) {
        await failTurn(reason, started);
      } finally {
        busyRef.current = false;
        actionIdRef.current = null;
        turnIdRef.current = null;
        setCanStop(false);
        setGenerating(null);
        setStreamText("");
      }
    },
    [replyOnce, refreshWorlds, onError, failTurn],
  );

  // 單次 GM 旁白＋點名（不含 busy 防護）：後端一次呼叫完成，旁白落 transcript，
  // 回傳下一位發言者（角色 id／玩家哨兵／null＝GM 沒點名）；失敗往外拋由呼叫端收尾
  const narrateOnce = useCallback(async (started: number): Promise<string | null> => {
    if (generation.current !== started) return null;
    noteChatStarted();
    setGenerating({ id: "", kind: "narration" });
    setStreamText("");
    responseTruncated.current = false;
    const turnId = beginTurn();
    setCanStop(true);
    const onDelta = new Channel<string>();
    onDelta.onmessage = (delta) => {
      if (generation.current === started) setStreamText((previous) => previous + delta);
    };
    // 正文落檔帶冪等鍵，失敗就原鍵重試；仍失敗才算這輪失敗（後端留著正文與表，下一回合開始時代落）
    const appendMain = async (event: TranscriptEvent) => {
      for (let attempt = 0; ; attempt += 1) {
        try {
          await appendEvent(event, started, { turnId, part: "main" });
          return;
        } catch (reason) {
          if (attempt >= 2 || String(reason).startsWith("AI_EMPTY_RESPONSE")) {
            mainLost.current = true;
            throw reason;
          }
          await new Promise((resolve) => setTimeout(resolve, 150 * (attempt + 1)));
        }
      }
    };
    const { text, raw, next, state_updates, arrived_characters, aborted, state_error } = await invoke<{
      text: string;
      raw: string | null;
      next: string | null;
      // 後端還沒上線這欄時是 undefined，當空陣列處理，別讓面板炸掉
      state_updates?: { path: string; value: string }[];
      // 這輪劇情帶出場的卡 id：併入本幕出場集合，auto_hidden 卡立刻從隱藏區移回主區
      arrived_characters?: string[];
      aborted?: boolean;
      // 狀態更新沒寫成：正文照落，提示這一輪的狀態可能沒更新
      state_error?: string | null;
    }>("gm_narrate", {
      worldId,
      turnId,
      onDelta,
      actionId: actionIdRef.current,
    }).finally(() => void showWorldInfoNotices(worldId));
    if (mainLost.current) {
      mainLost.current = false;
      await reload();
    }
    if (aborted) {
      await takeResponseTruncated();
      if (text.trim()) {
        await appendMain({
          ts: nowTs(),
          speaker_id: "",
          speaker_name: "GM",
          kind: "narration",
          text,
          truncated: true,
        });
        await markCliConnected();
      }
      return null;
    }
    const truncated = await takeResponseTruncated();
    const current = generation.current === started;
    if (current && arrived_characters && arrived_characters.length > 0) {
      onArrived(arrived_characters);
    }
    await appendMain({
      ts: nowTs(),
      speaker_id: "",
      speaker_name: "GM",
      kind: "narration",
      text,
      ...(raw ? { raw } : {}),
      ...(truncated ? { truncated: true } : {}),
    });
    if (state_error) onError(String(state_error));
    // 長文字欄（外貌、貼文…）改用一則系統事件記變動，不再每輪塞回提示詞——
    // 歷史會被兩條傳輸路每輪重播且吃快取，回合尾動態塊每輪重組、不落歷史
    const updates = state_updates ?? [];
    if (updates.length > 0) {
      await appendEvent({
        ts: nowTs(),
        speaker_id: "",
        speaker_name: "GM",
        kind: "system",
        text: updates.map((u) => `${u.path}：${u.value}`).join("\n"),
        marker: { type: "state_update" },
      }, started, { turnId, part: "state_update" });
    }
    // 換桌了：旁白已落進原桌，現在這桌的檯面與接力都不關它的事
    if (generation.current !== started) return null;
    await refreshState();
    await markCliConnected();
    return next;
  }, [noteChatStarted, worldId, onArrived, appendEvent, refreshState, markCliConnected, takeResponseTruncated, reload, onError]);

  // 簡易導演：GM 插入旁白（NewPlan §6.1、MVP 第 9 項）；一併回來的點名這裡不用，讓玩家自己決定下一步
  const gmNarrate = useCallback(async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    actionIdRef.current = crypto.randomUUID();
    appendFailed.current = false;
    const started = generation.current;
    onError("");
    try {
      await narrateOnce(started);
      await refreshWorlds();
    } catch (reason) {
      await failTurn(reason, started);
    } finally {
      busyRef.current = false;
      actionIdRef.current = null;
      turnIdRef.current = null;
      setCanStop(false);
      setGenerating(null);
      setStreamText("");
    }
  }, [narrateOnce, refreshWorlds, onError, failTurn]);

  // 簡易導演：GM 旁白＋點名→角色接話的接力，至「輪到玩家」、GM 沒點名或每回合上限停下（NewPlan §6.1）
  const gmAdvance = useCallback(async () => {
    if (!config || busyRef.current || castCount === 0) return;
    busyRef.current = true;
    actionIdRef.current = crypto.randomUUID();
    appendFailed.current = false;
    const started = generation.current;
    onError("");
    const max = Math.max(1, Number(config.preferences["max_round_speakers"]) || 3);
    try {
      for (let turn = 0; turn < max; turn += 1) {
        const next = await narrateOnce(started);
        // narrateOnce 之後、寫點名之前：停了或換桌了就不再點名、也不再接力
        if (stopRequested.current || next === null || generation.current !== started) break;
        // 輪到玩家：一樣留下點名紀錄（球在你手上），但不接話、就此停下
        if (next === PLAYER_SENTINEL) {
          // 玩家沒名字就存空字串，顯示與送 AI 時再照當下語系補稱呼
          await appendEvent({ ts: nowTs(), speaker_id: "", speaker_name: "GM", kind: "system", text: "", marker: { type: "gm_call", name: playerName ?? "" } }, started);
          break;
        }
        const name = metaOf(next)?.name ?? next;
        await appendEvent({ ts: nowTs(), speaker_id: "", speaker_name: "GM", kind: "system", text: "", marker: { type: "gm_call", name } }, started);
        // 點名寫檔期間按的停止要在這裡停。進 replyOnce 會 beginTurn 清掉旗標，角色就照樣接話。
        if (stopRequested.current) break;
        await replyOnce(next, started);
        if (stopRequested.current) break;
      }
      await refreshWorlds();
    } catch (reason) {
      await failTurn(reason, started);
    } finally {
      busyRef.current = false;
      actionIdRef.current = null;
      turnIdRef.current = null;
      setCanStop(false);
      setGenerating(null);
      setStreamText("");
    }
  }, [config, castCount, narrateOnce, playerName, appendEvent, metaOf, replyOnce, refreshWorlds, onError, failTurn]);

  // 請目前的發言對象接話：GM 以旁白回應（讀得到世界設定與全部角色卡），角色就點名接話
  const replyFromTarget = useCallback(async () => {
    if (busyRef.current) return;
    if (gmTargeted) await gmNarrate();
    else if (speaker) await requestReply(speaker);
  }, [gmTargeted, speaker, gmNarrate, requestReply]);

  // 打字送出（fromComposer）與卡片介面送出共用。只有打字送出會清輸入框、鎖輸入框，
  // 失敗時也只有它走「乾淨路徑」自動收回：玩家句落檔並拿到收據、之後沒回成，後端核對那一行
  // 確實還是檔尾才截掉，確定刪了才把原文放回輸入框；其他任何情況都不收、不放回，
  // 原文改放在失敗彈窗裡（見 .ai/plans/quota-insufficient-alert.md）
  const submitTurn = useCallback(
    async (raw: string, fromComposer: boolean) => {
      const text = raw.trim();
      if (busyRef.current) return;
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
          if (mainLost.current) {
            // 上一輪沒落成的 GM 回覆已由後端在這句之前代落：重讀，畫面順序跟磁碟一致
            mainLost.current = false;
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
        if (gmTargeted) await narrateOnce(started);
        else if (speaker) await replyOnce(speaker, started);
        await refreshWorlds();
      } catch (reason) {
        endWrite();
        if (!fromComposer) {
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
        busyRef.current = false;
        actionIdRef.current = null;
        turnIdRef.current = null;
        setCanStop(false);
        setGenerating(null);
        setStreamText("");
        if (fromComposer) setSending(false);
      }
    },
    [gmTargeted, speaker, gmNarrate, requestReply, worldId, scene, playerName, onError, narrateOnce, replyOnce, refreshWorlds, failTurn, discardPlayer, refreshState, reload, beginWrite],
  );

  const submitText = useCallback((raw: string) => submitTurn(raw, false), [submitTurn]);

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
    stopRequested.current = false;
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

  const visibleNotices = useMemo(
    () => notices.filter((entry) => entry.gen === generation.current),
    [notices, worldId, scene],
  );

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
      gmNarrate,
      gmAdvance,
      replyFromTarget,
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
      gmNarrate,
      gmAdvance,
      replyFromTarget,
      canStop,
      stopResponse,
      beginNarration,
      endNarration,
    ],
  );
}
