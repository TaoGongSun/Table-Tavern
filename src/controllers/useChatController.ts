// 聊天 controller：這一幕的逐字稿、收回堆疊、生成中狀態與輸入框，以及玩家送出、
// 角色接話、GM 旁白與推進接力的整條流程。所有權從 App() 搬過來，行為與依賴陣列照舊。
import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { AppConfig, TranscriptEvent } from "../backend-contracts";
import { CharacterMeta } from "../card-model";

// GM 點到玩家時後端回這個代號（transport.rs 的 PLAYER_SENTINEL），收到就把發言權交回給玩家
const PLAYER_SENTINEL = "__PLAYER__";

// 保溫 ping（prompt-cache-optimization 包 7）：快取只活五分鐘，玩家慢慢想的時候先讀一次
// 既有快取把壽命重新計時，代價約為讓它過期重建的十二分之一。連三次（約十二分鐘）都沒等到
// 玩家推進就收手改提示換幕——人真的離開時，長紀錄每次回來都要全額重建，那時短紀錄才便宜。
const KEEPALIVE_TICK_MS = 30 * 1000;
const KEEPALIVE_AFTER_MS = 3.5 * 60 * 1000;
const KEEPALIVE_MAX_PINGS = 3;

function nowTs() {
  return new Date().toISOString();
}

export interface ChatController {
  /** 這一幕已落檔的逐字稿 */
  events: TranscriptEvent[];
  /** 是誰在生成、以哪種形式；id 空字串＝GM */
  generating: { id: string; kind: "dialogue" | "narration" } | null;
  /** `generating !== null`：桌次操作與按鈕的忙碌判斷都讀這個 */
  busy: boolean;
  /** 串流到一半的文字 */
  streamText: string;
  input: string;
  setInput: (value: string) => void;
  /** 保溫連發到上限還沒等到玩家推進：畫面據此提示換幕 */
  awayTooLong: boolean;
  /** 收回過、且還停在同一桌同一幕，才給復原 */
  canRestore: boolean;
  /** 換桌：整份換掉這一幕的逐字稿 */
  hydrate: (transcript: TranscriptEvent[]) => void;
  /** 檯面被外部改動（復原匯入收掉開場白）後重讀這一幕 */
  reload: () => Promise<void>;
  /** 貼出開場白；true＝真的貼上檯面了，呼叫端據此收掉開場白面板（失敗時面板留著） */
  postOpening: (text: string) => Promise<boolean>;
  undoLast: () => Promise<void>;
  restoreUndone: () => Promise<void>;
  send: (event: FormEvent<HTMLFormElement>) => Promise<void>;
  submitText: (raw: string) => Promise<void>;
  gmNarrate: () => Promise<void>;
  gmAdvance: () => Promise<void>;
  /** 請目前的發言對象接話 */
  replyFromTarget: () => Promise<void>;
  /** 換幕這類 App 自己跑的長工作：期間畫面顯示 GM 正在生成 */
  beginNarration: () => void;
  endNarration: () => void;
  /** 玩家真的推進了一步：保溫節奏重新開始，離開提示收掉 */
  noteTurnDone: () => void;
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
}): ChatController {
  const [events, setEvents] = useState<TranscriptEvent[]>([]);
  // 這一輪收回的那幾句，後收的疊在最上面（復原一次拿一則，順序自然還原）。
  // 記下當時的桌與幕，換桌換幕後整疊自動失效（比對不上就不顯示），免得放回錯的地方
  const [undone, setUndone] = useState<{
    worldId: string;
    scene: number;
    events: TranscriptEvent[];
  } | null>(null);
  // 連按時前一次的寫檔還沒回來就再按，兩次會讀到同一份舊狀態而重複收回／放回同一則；
  // 用旗標讓同一時間只跑一次（寫檔是毫秒級，擋掉的那下感覺不出來）
  const undoBusy = useRef(false);
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

  // 收回過、且還停在同一桌同一幕，才給復原（換桌換幕就當這次收回已成定局）
  const canRestore = undone !== null && undone.worldId === worldId && undone.scene === scene;

  const hydrate = useCallback((transcript: TranscriptEvent[]) => {
    setEvents(transcript);
  }, []);

  const reload = useCallback(async () => {
    setEvents(await invoke<TranscriptEvent[]>("read_transcript", { worldId, scene }));
  }, [worldId, scene]);

  const appendEvent = useCallback(
    async (event: TranscriptEvent) => {
      await invoke("append_transcript", { worldId, scene, event });
      setEvents((previous) => [...previous, event]);
      // 桌上一有新內容，收回的那幾句就不能再放回去了——位置已經被後面的話蓋掉
      setUndone(null);
    },
    [worldId, scene],
  );

  const postOpening = useCallback(
    async (text: string) => {
      onError("");
      try {
        const event = await invoke<TranscriptEvent>("post_opening", { worldId, scene, ts: nowTs(), text });
        setEvents((previous) => [...previous, event]);
        setUndone(null);
        await refreshState();
        return true;
      } catch (reason) {
        onError(String(reason));
        return false;
      }
    },
    [worldId, scene, refreshState, onError],
  );

  // 收回上一句：一次砍一則、可連按往回收，收到這一幕見底就停（不動上一幕）
  const undoLast = useCallback(async () => {
    if (generating !== null || events.length === 0 || undoBusy.current) return;
    undoBusy.current = true;
    onError("");
    const last = events[events.length - 1];
    try {
      if (!(await invoke<boolean>("pop_transcript", { worldId, scene }))) return;
      setEvents((previous) => previous.slice(0, -1));
      setUndone((previous) =>
        previous && previous.worldId === worldId && previous.scene === scene
          ? { ...previous, events: [...previous.events, last] }
          : { worldId, scene, events: [last] },
      );
      await refreshState();
    } catch (reason) {
      onError(String(reason));
    } finally {
      undoBusy.current = false;
    }
  }, [generating, events, worldId, scene, refreshState, onError]);

  // 復原一次放回一則，可連按把整輪收回逐則倒回去。
  // 這裡不走 appendEvent——放回舊句不該把剩下那幾句一起作廢，只消耗疊頂那一則
  const restoreUndone = useCallback(async () => {
    if (!undone || !canRestore || generating !== null || undoBusy.current) return;
    undoBusy.current = true;
    const event = undone.events[undone.events.length - 1];
    onError("");
    try {
      await invoke("append_transcript", { worldId, scene, event });
      setEvents((previous) => [...previous, event]);
      setUndone((previous) =>
        previous && previous.events.length > 1
          ? { ...previous, events: previous.events.slice(0, -1) }
          : null,
      );
      await refreshState();
    } catch (reason) {
      onError(String(reason));
    } finally {
      undoBusy.current = false;
    }
  }, [undone, canRestore, generating, worldId, scene, refreshState, onError]);

  // 玩家真的推進了一步：保溫節奏重新開始，離開提示收掉
  const noteTurnDone = useCallback(() => {
    lastTurnAt.current = Date.now();
    pingCount.current = 0;
    keepaliveOff.current = false;
    setAwayTooLong(false);
  }, []);

  // 保溫 ping：視窗在前景且距上次推進夠久才發，連三次都沒等到玩家就收手改提示換幕。
  // 視窗不在前景一律不發——人不在還持續扣錢是最糟的情況。
  // 生成中狀態走 ref：計時器整桌只掛一次，不隨每次生成重訂閱。
  useEffect(() => {
    if (!worldId) return;
    const timer = setInterval(async () => {
      if (keepaliveOff.current || generatingRef.current !== null) return;
      if (Date.now() - lastTurnAt.current < KEEPALIVE_AFTER_MS) return;
      if (pingCount.current >= KEEPALIVE_MAX_PINGS) {
        setAwayTooLong(true);
        return;
      }
      if (!document.hasFocus()) return;
      try {
        const lanes = await invoke<number>("keepalive_lanes", { worldId });
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
  }, [worldId]);

  // 單次角色接話（不含 busy 防護），供手動點名與 GM 推進共用；失敗往外拋由呼叫端收尾
  const replyOnce = useCallback(
    async (characterId: string) => {
      noteChatStarted();
      setGenerating({ id: characterId, kind: "dialogue" });
      setStreamText("");
      const onDelta = new Channel<string>();
      onDelta.onmessage = (delta) => setStreamText((previous) => previous + delta);
      const full = await invoke<string>("chat_with_character", {
        worldId,
        characterId,
        onDelta,
      });
      const name = metaOf(characterId)?.name ?? "";
      await appendEvent({ ts: nowTs(), speaker_id: characterId, speaker_name: name, kind: "dialogue", text: full });
      await markCliConnected();
      noteTurnDone();
    },
    [noteChatStarted, worldId, metaOf, appendEvent, markCliConnected, noteTurnDone],
  );

  // 點名指定角色接話；也是「請 X 發言」按鈕的入口（NewPlan §9、MVP 第 8 項）
  const requestReply = useCallback(
    async (characterId: string) => {
      if (!characterId || generating !== null) return;
      onError("");
      try {
        await replyOnce(characterId);
        await refreshWorlds();
      } catch (reason) {
        onError(String(reason));
      } finally {
        setGenerating(null);
        setStreamText("");
      }
    },
    [generating, replyOnce, refreshWorlds, onError],
  );

  // 單次 GM 旁白＋點名（不含 busy 防護）：後端一次呼叫完成，旁白落 transcript，
  // 回傳下一位發言者（角色 id／玩家哨兵／null＝GM 沒點名）；失敗往外拋由呼叫端收尾
  const narrateOnce = useCallback(async (): Promise<string | null> => {
    noteChatStarted();
    setGenerating({ id: "", kind: "narration" });
    setStreamText("");
    const onDelta = new Channel<string>();
    onDelta.onmessage = (delta) => setStreamText((previous) => previous + delta);
    const { text, raw, next, state_updates, arrived_characters } = await invoke<{
      text: string;
      raw: string | null;
      next: string | null;
      // 後端還沒上線這欄時是 undefined，當空陣列處理，別讓面板炸掉
      state_updates?: { path: string; value: string }[];
      // 這輪劇情帶出場的卡 id：併入本幕出場集合，auto_hidden 卡立刻從隱藏區移回主區
      arrived_characters?: string[];
    }>("gm_narrate", {
      worldId,
      onDelta,
    });
    if (arrived_characters && arrived_characters.length > 0) {
      onArrived(arrived_characters);
    }
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
    await refreshState();
    await markCliConnected();
    noteTurnDone();
    return next;
  }, [noteChatStarted, worldId, onArrived, appendEvent, refreshState, markCliConnected, noteTurnDone]);

  // 簡易導演：GM 插入旁白（NewPlan §6.1、MVP 第 9 項）；一併回來的點名這裡不用，讓玩家自己決定下一步
  const gmNarrate = useCallback(async () => {
    if (generating !== null) return;
    onError("");
    try {
      await narrateOnce();
      await refreshWorlds();
    } catch (reason) {
      onError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }, [generating, narrateOnce, refreshWorlds, onError]);

  // 簡易導演：GM 旁白＋點名→角色接話的接力，至「輪到玩家」、GM 沒點名或每回合上限停下（NewPlan §6.1）
  const gmAdvance = useCallback(async () => {
    if (!config || generating !== null || castCount === 0) return;
    onError("");
    const max = Math.max(1, Number(config.preferences["max_round_speakers"]) || 3);
    try {
      for (let turn = 0; turn < max; turn += 1) {
        const next = await narrateOnce();
        if (next === null) break;
        // 輪到玩家：一樣留下點名紀錄（球在你手上），但不接話、就此停下
        if (next === PLAYER_SENTINEL) {
          const you = playerName || t("playerLabel");
          await appendEvent({ ts: nowTs(), speaker_id: "", speaker_name: "GM", kind: "system", text: t("gmCallOn", { name: you }) });
          break;
        }
        const name = metaOf(next)?.name ?? next;
        await appendEvent({ ts: nowTs(), speaker_id: "", speaker_name: "GM", kind: "system", text: t("gmCallOn", { name }) });
        await replyOnce(next);
      }
      await refreshWorlds();
    } catch (reason) {
      onError(String(reason));
    } finally {
      setGenerating(null);
      setStreamText("");
    }
  }, [config, generating, castCount, narrateOnce, playerName, appendEvent, metaOf, replyOnce, refreshWorlds, onError]);

  // 請目前的發言對象接話：GM 以旁白回應（讀得到世界設定與全部角色卡），角色就點名接話
  const replyFromTarget = useCallback(async () => {
    if (gmTargeted) await gmNarrate();
    else if (speaker) await requestReply(speaker);
  }, [gmTargeted, speaker, gmNarrate, requestReply]);

  const submitText = useCallback(
    async (raw: string) => {
      const text = raw.trim();
      if (generating !== null) return;
      // 卡片只按了 /trigger（沒帶文字）＝直接要對象接話，不留玩家發言
      if (!text) {
        await replyFromTarget();
        return;
      }
      onError("");
      setInput("");
      try {
        await appendEvent({ ts: nowTs(), speaker_id: "", speaker_name: playerName || t("playerLabel"), kind: "player", text });
      } catch (reason) {
        onError(String(reason));
        return;
      }
      // 沒指定對象＝只把這句留在桌上（描述動作或對全場說），不點名任何人接話
      await replyFromTarget();
    },
    [generating, replyFromTarget, appendEvent, playerName, onError],
  );

  const send = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      await submitText(input);
    },
    [submitText, input],
  );

  // 換幕／重生摘要跑在 App 那頭，但畫面上那段「GM 正在生成」屬於這裡
  const beginNarration = useCallback(() => {
    setGenerating({ id: "", kind: "narration" });
    setStreamText("");
  }, []);

  const endNarration = useCallback(() => {
    setGenerating(null);
    setStreamText("");
  }, []);

  return useMemo(
    () => ({
      events,
      generating,
      busy: generating !== null,
      streamText,
      input,
      setInput,
      awayTooLong,
      canRestore,
      hydrate,
      reload,
      postOpening,
      undoLast,
      restoreUndone,
      send,
      submitText,
      gmNarrate,
      gmAdvance,
      replyFromTarget,
      beginNarration,
      endNarration,
      noteTurnDone,
    }),
    [
      events,
      generating,
      streamText,
      input,
      awayTooLong,
      canRestore,
      hydrate,
      reload,
      postOpening,
      undoLast,
      restoreUndone,
      send,
      submitText,
      gmNarrate,
      gmAdvance,
      replyFromTarget,
      beginNarration,
      endNarration,
      noteTurnDone,
    ],
  );
}
