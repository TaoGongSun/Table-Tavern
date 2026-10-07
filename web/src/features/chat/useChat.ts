// 一場對話（只在記憶體，包 4 才存檔）：送出互斥、串流顯示、停止、取消後不寫入、換模提示、額度導流。
import { useCallback, useEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import type { CharacterCardData } from "../cards/sample-card";
import { quotaBlocksSending } from "../funnel/quota";
import { fetchFreeDaily } from "../openrouter/openrouter-api";
import { isDailyExhausted, NO_FREE_MODEL, runSmartCall, type CallOutcome, type FailoverNotice } from "../openrouter/smart-call";
import { streamChat } from "../openrouter/stream-chat";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { resolveTurn, type ChatEntry } from "./chat-turn";
import { explainError } from "./error-text";
import { buildMessages, replaceNameMacros } from "./prompt";

let counter = 0;
const newId = () => `m${Date.now().toString(36)}${(counter += 1)}`;
const nowSecs = () => Math.floor(Date.now() / 1000);
const CANCELLED = Symbol("cancelled");

export function openingEntries(card: CharacterCardData, userName: string): ChatEntry[] {
  const text = replaceNameMacros(card.first_mes, card.name, userName).trim();
  return text ? [{ id: newId(), role: "char", text, opening: true }] : [];
}

export interface ChatController {
  entries: ChatEntry[];
  input: string;
  setInput: (value: string) => void;
  busy: boolean;
  /** 正在串流的這一則（還沒落進逐字稿）。 */
  streaming: string;
  error: string | null;
  failover: FailoverNotice | null;
  send: () => Promise<void>;
  stop: () => void;
}

export function useChat(card: CharacterCardData, session: OpenRouterSession): ChatController {
  const userName = t("defaultUserName");
  const [entries, setEntries] = useState<ChatEntry[]>(() => openingEntries(card, userName));
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

  const send = useCallback(async () => {
    const text = input.trim();
    const apiKey = session.apiKey;
    // 送出互斥：同一時間只跑一個回合
    if (!text || !apiKey || busyRef.current || quotaBlocksSending(session.quota)) return;
    // 第一個 await 之前就備好取消與世代：送出前的 /key 查詢期間按停止或卸載也算數
    busyRef.current = true;
    const controller = new AbortController();
    abortRef.current = controller;
    const generation = (generationRef.current += 1);
    const cancelled = () => controller.signal.aborted || generationRef.current !== generation;
    setBusy(true);
    setError(null);
    setFailover(null);
    try {
      // 送出前先看今日免費次數（同桌面版 smart_free::prepare_call）；用完就不送、原文留在輸入框
      const daily = await fetchFreeDaily(session.deps, apiKey);
      // 這段被取消＝不送、不清輸入框、不落玩家句
      if (cancelled()) return;
      session.quotaEvent({ type: "key-info", daily });
      if (daily?.kind === "counted" && daily.remaining <= 0) return;

      setInput("");
      const userEntry: ChatEntry = { id: newId(), role: "user", text };
      const before = [...entriesRef.current, userEntry];
      setEntries(before);

      let outcome: CallOutcome;
      try {
        await session.pool.refresh(apiKey, nowSecs());
        // refresh 期間被取消：不組 plan、不碰選模狀態，照「取消未完成回合」收回玩家句
        if (cancelled()) throw CANCELLED;
        const messages = buildMessages(card, userName, before);
        const plan = session.pool.plan(messages.map((message) => message.content), nowSecs());
        outcome = await runSmartCall(plan, session.runtime, {
          signal: controller.signal,
          now: nowSecs,
          dailyRemaining: async () => {
            const result = await fetchFreeDaily(session.deps, apiKey);
            return result?.kind === "counted" ? result.remaining : null;
          },
          send: (model) => {
            setStreaming("");
            return streamChat({
              fetch: session.deps.fetch,
              apiBase: session.deps.apiBase,
              apiKey,
              model,
              messages,
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

      const result = resolveTurn(before, userEntry, outcome, newId);
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
  }, [input, session, card, userName]);

  return { entries, input, setInput, busy, streaming, error, failover, send, stop };
}
