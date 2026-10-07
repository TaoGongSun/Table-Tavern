// 一場對話（只在記憶體，包 4 才存檔）：送出互斥、串流顯示、停止、取消後不寫入、換模提示、額度導流，
// 以及最後一則的重新生成／編輯／刪除。訊息文字在各時機的 ST 處理見 st-text.ts，提示組裝見 prompt.ts。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { PlayCard } from "../cards/play-card";
import { quotaBlocksSending } from "../funnel/quota";
import { fetchFreeDaily } from "../openrouter/openrouter-api";
import { isDailyExhausted, NO_FREE_MODEL, runSmartCall, type CallOutcome, type FailoverNotice } from "../openrouter/smart-call";
import { streamChat } from "../openrouter/stream-chat";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { RESERVED_OUTPUT_TOKENS } from "../openrouter/catalog";
import { commitVariables, copyVariables, createChatVariables, type VariableMap } from "../sillytavern/variables";
import { deleteLast, regenerateBase, replaceLast, resolveTurn, type ChatEntry, type PendingTurn } from "./chat-turn";
import { explainError } from "./error-text";
import { composePrompt } from "./prompt";
import { editedText, openingText, replyText, userText, type ChatSetup } from "./st-text";

let counter = 0;
const newId = () => `m${Date.now().toString(36)}${(counter += 1)}`;
const nowSecs = () => Math.floor(Date.now() / 1000);
const CANCELLED = Symbol("cancelled");

/** ST 的 global 變數跨對話共用；網頁版目前只活在這個分頁。 */
const GLOBAL_VARIABLES: VariableMap = {};

/** 開一桌要的東西：卡、玩家名、選哪個開場白（null＝不放開場白）。 */
export interface GameSetup {
  card: PlayCard;
  userName: string;
  openingIndex: number | null;
}

export function openingEntries(setup: ChatSetup, openingIndex: number | null): ChatEntry[] {
  const raw = openingIndex === null ? undefined : setup.card.openings[openingIndex];
  if (raw === undefined) return [];
  const text = openingText(setup, raw).trim();
  return text ? [{ id: newId(), role: "char", text, opening: true, sentAt: Date.now() }] : [];
}

export interface ChatController {
  setup: ChatSetup;
  entries: ChatEntry[];
  input: string;
  setInput: (value: string) => void;
  busy: boolean;
  /** 正在串流的這一則（還沒落進逐字稿）。 */
  streaming: string;
  error: string | null;
  failover: FailoverNotice | null;
  send: () => Promise<void>;
  regenerate: () => Promise<void>;
  canRegenerate: boolean;
  editLast: (text: string) => void;
  deleteLast: () => void;
  stop: () => void;
}

export function useChat(game: GameSetup, session: OpenRouterSession): ChatController {
  const setup = useMemo<ChatSetup>(
    () => ({
      card: game.card,
      userName: game.userName,
      variables: createChatVariables({}, GLOBAL_VARIABLES),
      chatId: `web-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
    }),
    [game.card, game.userName],
  );
  const [entries, setEntries] = useState<ChatEntry[]>(() => openingEntries(setup, game.openingIndex));
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [streaming, setStreaming] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [failover, setFailover] = useState<FailoverNotice | null>(null);
  const busyRef = useRef(false);
  const abortRef = useRef<AbortController | null>(null);
  /** 回合世代：卸載時推進，晚到的結果認得出自己已作廢。 */
  const generationRef = useRef(0);
  const entriesRef = useRef(entries);
  entriesRef.current = entries;

  const stop = useCallback(() => abortRef.current?.abort(), []);
  // 換卡、登出時畫面卸載：在途的回合一併取消，不再替玩家多花一次免費額度
  useEffect(
    () => () => {
      generationRef.current += 1;
      abortRef.current?.abort();
    },
    [],
  );

  /**
   * 一個回合：送出前查今日免費次數 → `begin` 把逐字稿改成這輪的起點（回 null＝不送）→ 選模串流 → 收尾。
   * 第一個 await 之前就備好取消與世代：查詢期間按停止或卸載也算數，逐字稿與輸入框都不動。
   */
  const runTurn = useCallback(
    async (begin: () => { before: ChatEntry[]; pending: PendingTurn } | null, generationType: "normal" | "regenerate") => {
      const apiKey = session.apiKey;
      if (!apiKey || busyRef.current || quotaBlocksSending(session.quota)) return;
      busyRef.current = true;
      const controller = new AbortController();
      abortRef.current = controller;
      const generation = (generationRef.current += 1);
      const cancelled = () => controller.signal.aborted || generationRef.current !== generation;
      setBusy(true);
      setError(null);
      setFailover(null);
      try {
        // 同桌面版 smart_free::prepare_call；用完就不送、原文留在輸入框
        const daily = await fetchFreeDaily(session.deps, apiKey);
        if (cancelled()) return;
        session.quotaEvent({ type: "key-info", daily });
        if (daily?.kind === "counted" && daily.remaining <= 0) return;
        const started = begin();
        if (!started) return;
        const { pending } = started;
        const input = pending.kind === "send" ? pending.rawInput : "";
        setEntries(started.before);
        // 每次組提示都從回合開頭的變數副本起算；真正派送的那一次才把副作用（setvar 等）與第 0 則寫回落地，
        // 試組與換模前的那一發不重複提交
        const snapshot = copyVariables(setup.variables);
        const compose = (model?: string) => {
          const variables = copyVariables(snapshot);
          const limits =
            model === undefined ? undefined : { maxContext: session.pool.contextLength(model), maxResponse: RESERVED_OUTPUT_TOKENS };
          return { variables, ...composePrompt({ ...setup, variables }, started.before, { generationType, input, model, limits }) };
        };
        /** 真正派送出去的最後一發（換模第二發會蓋掉第一發）。 */
        const dispatched: { turn: ReturnType<typeof compose> | null } = { turn: null };

        let outcome: CallOutcome;
        try {
          await session.pool.refresh(apiKey, nowSecs());
          // refresh 期間被取消：不組 plan、不碰選模狀態，照「取消未完成回合」收尾
          if (cancelled()) throw CANCELLED;
          // 先試組一次挑模型；每一發再用那一發的模型與它的上限重組
          const plan = session.pool.plan(compose().messages.map((message) => message.content), nowSecs());
          outcome = await runSmartCall(plan, session.runtime, {
            signal: controller.signal,
            now: nowSecs,
            dailyRemaining: async () => {
              const result = await fetchFreeDaily(session.deps, apiKey);
              return result?.kind === "counted" ? result.remaining : null;
            },
            send: (model) => {
              const turn = compose(model);
              dispatched.turn = turn;
              commitVariables(setup.variables, turn.variables);
              setEntries(turn.entries);
              setStreaming("");
              return streamChat({
                fetch: session.deps.fetch,
                apiBase: session.deps.apiBase,
                apiKey,
                model,
                messages: turn.messages,
                signal: controller.signal,
                // 取消後一個字都不再顯示
                onDelta: (delta) => !cancelled() && setStreaming((shown) => shown + delta),
              });
            },
          });
        } catch (reason) {
          if (reason === CANCELLED) {
            outcome = { kind: "aborted", text: "", failover: null };
          } else {
            const display = reason instanceof Error && reason.message === NO_FREE_MODEL ? NO_FREE_MODEL : String(reason);
            outcome = { kind: "error", display, failure: null, cls: null, daily: false, failover: null };
          }
        }
        // 畫面已卸載：結果不再寫回
        if (generationRef.current !== generation) return;

        const before = dispatched.turn?.entries ?? started.before;
        const result = resolveTurn(before, pending, outcome, newId, (text) => replyText(setup, before, text));
        setEntries(result.entries);
        setStreaming("");
        setFailover(outcome.failover);
        if (result.restoreInput !== null) setInput(result.restoreInput);
        if (isDailyExhausted(outcome)) session.quotaEvent({ type: "daily-exhausted-error" });
        else if (result.error !== null) setError(explainError(result.error));
        // 更新今日免費次數（/key 不佔次數）
        void session.refreshQuota();
      } finally {
        if (abortRef.current === controller) abortRef.current = null;
        busyRef.current = false;
        setBusy(false);
      }
    },
    [session, setup],
  );

  const send = useCallback(async () => {
    const raw = input.trim();
    if (!raw) return;
    await runTurn(() => {
      const current = entriesRef.current;
      const userEntry: ChatEntry = { id: newId(), role: "user", text: userText(setup, current, raw), sentAt: Date.now() };
      setInput("");
      return { before: [...current, userEntry], pending: { kind: "send", userEntry, rawInput: raw } };
    }, "normal");
  }, [input, runTurn, setup]);

  const regenerate = useCallback(async () => {
    if (!regenerateBase(entriesRef.current)) return;
    await runTurn(() => {
      const base = regenerateBase(entriesRef.current);
      return base && { before: base.before, pending: { kind: "regenerate", replaced: base.replaced } };
    }, "regenerate");
  }, [runTurn]);

  const editLast = useCallback(
    (text: string) => {
      if (busyRef.current) return;
      const current = entriesRef.current;
      const last = current[current.length - 1];
      if (!last) return;
      setEntries(replaceLast(current, editedText(setup, current, last.role, text)));
    },
    [setup],
  );

  const removeLast = useCallback(() => {
    if (busyRef.current) return;
    setEntries(deleteLast(entriesRef.current));
  }, []);

  return {
    setup,
    entries,
    input,
    setInput,
    busy,
    streaming,
    error,
    failover,
    send,
    regenerate,
    canRegenerate: regenerateBase(entries) !== null,
    editLast,
    deleteLast: removeLast,
    stop,
  };
}
