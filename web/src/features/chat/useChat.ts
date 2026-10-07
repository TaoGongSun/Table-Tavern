// 一場對話：送出互斥、串流顯示、停止、取消後不寫入、換模提示、額度導流，以及最後一則的重新生成／
// 編輯／刪除。訊息文字在各時機的 ST 處理見 st-text.ts，提示組裝見 prompt.ts。逐字稿每次定下來（不在回合
// 中）就自動存進這個瀏覽器的存檔庫；新開的桌等玩家第一次開口才佔一格存檔。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { PlayCard } from "../cards/play-card";
import { quotaBlocksSending } from "../funnel/quota";
import { fetchFreeDaily } from "../openrouter/openrouter-api";
import {
  isDailyExhausted,
  NO_FREE_MODEL,
  PROMPT_EXCEEDS_CONTEXT,
  runSmartCall,
  type CallOutcome,
  type FailoverNotice,
} from "../openrouter/smart-call";
import { streamChat } from "../openrouter/stream-chat";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { RESERVED_OUTPUT_TOKENS } from "../openrouter/catalog";
import { EMPTY_CARRY, toWebSave, type SaveCarry } from "../saves/web-save-codec";
import { forgetPendingInput, readPendingInput, rememberPendingInput } from "../saves/pending-input";
import { newSaveId, type SaveStore } from "../saves/save-store";
import { toStChat } from "../saves/st-chat";
import { substituteParams } from "../sillytavern/substitute";
import { commitVariables, copyVariables, createChatVariables, type VariableMap } from "../sillytavern/variables";
import { parseWebSave, type WebSave } from "@desktop/shared/contracts/web-save/web-save";
import { deleteLast, regenerateBase, replaceLast, resolveTurn, type ChatEntry, type PendingTurn } from "./chat-turn";
import { t } from "../../i18n";
import { explainError } from "./error-text";
import { composePrompt } from "./prompt";
import { messageTokenCounter } from "../sillytavern/tokens";
import { editedText, macroContext, openingText, replyText, userText, type ChatSetup } from "./st-text";

let counter = 0;
const newId = () => `m${Date.now().toString(36)}${(counter += 1)}`;
const nowSecs = () => Math.floor(Date.now() / 1000);
const CANCELLED = Symbol("cancelled");

/** ST 的 global 變數跨對話共用：開站時從存檔庫讀回、自動存檔時寫回（D29）；從存檔接著玩時只補缺。 */
export const GLOBAL_VARIABLES: VariableMap = {};

/** 開一桌要的東西：卡、玩家名、選哪個開場白（null＝不放開場白）。 */
export interface GameSetup {
  card: PlayCard;
  userName: string;
  openingIndex: number | null;
  /** 這桌在瀏覽器存檔庫裡的位置；新開的桌不給，開桌時配一個 */
  saveId?: string;
  /** 從存檔接著玩：逐字稿、這段對話的變數、網頁版還用不到但要原樣帶回的欄位 */
  resume?: { entries: ChatEntry[]; local: VariableMap; carry: SaveCarry };
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
  /** 這桌現在的樣子，照桌檔契約 v1 */
  exportSave: () => WebSave;
  /** 匯出成 SillyTavern 聊天檔（.jsonl，有損） */
  exportStChat: () => string;
  /** 最近一次自動存檔失敗（瀏覽器不讓存、空間滿了） */
  saveFailed: boolean;
}

export function useChat(game: GameSetup, session: OpenRouterSession, saves: SaveStore | null = null): ChatController {
  const [saveId] = useState(() => game.saveId ?? newSaveId());
  const setup = useMemo<ChatSetup>(
    () => ({
      card: game.card,
      userName: game.userName,
      variables: createChatVariables(structuredClone(game.resume?.local ?? {}), GLOBAL_VARIABLES),
      chatId: `web-${saveId}`,
    }),
    [game.card, game.userName, game.resume, saveId],
  );
  const [entries, setEntries] = useState<ChatEntry[]>(() => game.resume?.entries ?? openingEntries(setup, game.openingIndex));
  const [saveFailed, setSaveFailed] = useState(false);
  // 上次在回合進行中重新整理（D28）：那一句放回輸入框
  const [input, setInput] = useState(() => (game.resume ? readPendingInput(saveId) : null) ?? "");
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
  /** 把這桌現在的樣子寫進存檔庫（下面定義；回合開始時要先用） */
  const writeSaveRef = useRef<() => Promise<boolean>>(async () => true);
  /** 這桌已經佔了一格存檔 */
  const slotRef = useRef(game.resume !== undefined);

  useEffect(() => {
    if (game.resume) forgetPendingInput(saveId);
  }, [game.resume, saveId]);

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
        // 新開的桌第一次開口（D27）：開口前的樣子先存成一格，寫成了才送模——回合中重新整理才有完整回合可退
        // （D28）。存不進去就不送，原句留在輸入框
        if (!slotRef.current) {
          const landed = await writeSaveRef.current();
          if (cancelled()) return;
          if (!landed) {
            setError(t("autosaveFirstFailed"));
            return;
          }
        }
        const started = begin();
        if (!started) return;
        const { pending } = started;
        // 回合中的那一句不在存檔裡：另記草稿，重新整理後放回輸入框（D28）
        if (pending.kind === "send") rememberPendingInput(saveId, pending.rawInput);
        const input = pending.kind === "send" ? pending.rawInput : "";
        setEntries(started.before);
        // 每支模型照它的上限與 token 估算組一次提示（D23），都從回合開頭的變數副本起算；真正派送的那一次才把
        // 副作用（setvar 等）與第 0 則寫回落地，選模時試組與換模前的那一發不重複提交
        const snapshot = copyVariables(setup.variables);
        const composed = new Map<string, ReturnType<typeof composeFor>>();
        const composeFor = (model: string) => {
          const variables = copyVariables(snapshot);
          const limits = { maxContext: session.pool.contextLength(model), maxResponse: RESERVED_OUTPUT_TOKENS };
          const countTokens = messageTokenCounter(model, session.pool.tokenizer(model));
          return { variables, ...composePrompt({ ...setup, variables }, started.before, { generationType, input, model, limits, countTokens }) };
        };
        const compose = (model: string) => {
          const turn = composed.get(model) ?? composeFor(model);
          composed.set(model, turn);
          return turn;
        };
        /** 選模時有模型因固定段落超過它的預算被跳過 */
        let overflowSeen = false;
        const holds = (model: string) => {
          const overflow = compose(model).overflow;
          if (overflow) overflowSeen = true;
          return !overflow;
        };
        /** 真正派送出去的最後一發（換模第二發會蓋掉第一發）。 */
        const dispatched: { turn: ReturnType<typeof compose> | null } = { turn: null };

        let outcome: CallOutcome;
        try {
          await session.pool.refresh(apiKey, nowSecs());
          // refresh 期間被取消：不組 plan、不碰選模狀態，照「取消未完成回合」收尾
          if (cancelled()) throw CANCELLED;
          // 選模只挑放得下固定段落的模型；每一發用那一發的模型與它的上限組的提示
          const plan = session.pool.plan(holds, nowSecs());
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
            const known = reason instanceof Error && [NO_FREE_MODEL, PROMPT_EXCEEDS_CONTEXT].includes(reason.message);
            const display = known ? (reason as Error).message : String(reason);
            outcome = { kind: "error", display, failure: null, cls: null, daily: false, failover: null };
          }
        }
        // 沒有模型放得下、而且有模型是因為固定段落超過預算被跳過：照 ST 的「Mandatory prompts exceed the
        // context size」說明，不當成沒有免費模型
        if (outcome.kind === "error" && outcome.display === NO_FREE_MODEL && overflowSeen) {
          outcome = { ...outcome, display: PROMPT_EXCEEDS_CONTEXT };
        }
        // 畫面已卸載：結果不再寫回（草稿留著，接著玩時放回輸入框）
        if (generationRef.current !== generation) return;
        forgetPendingInput(saveId);

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
    [session, setup, saveId],
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

  const exportSave = useCallback(
    () =>
      toWebSave({
        card: game.card,
        userName: game.userName,
        openingIndex: game.openingIndex,
        entries: entriesRef.current,
        local: setup.variables.local.values,
        global: setup.variables.global.values,
        globalWritten: setup.variables.global.written,
        carry: game.resume?.carry ?? EMPTY_CARRY,
        exportedAt: Date.now(),
      }),
    [game, setup],
  );

  const exportStChat = useCallback(() => {
    const current = entriesRef.current;
    // 開場白照 ST 顯示前代換巨集；用變數副本，匯出不改這桌的變數
    const preview = { ...setup, variables: copyVariables(setup.variables) };
    const texts = current.map((entry, index) =>
      index === 0 && entry.role === "char" ? substituteParams(entry.text, macroContext(preview, current)) : entry.text,
    );
    return toStChat({ userName: game.userName, characterName: game.card.text.name, texts, entries: current, createdAt: Date.now() });
  }, [game, setup]);

  // 自動存檔：回合中不存（逐字稿還會變，D28 重新整理就退回上次完整回合）；新開的桌在玩家第一次開口前不佔
  // 存檔（D27），佔了之後每次都存。同一格的寫入排成一條：前一筆寫完才寫下一筆，晚到的舊回合蓋不掉新回合
  // （存檔庫另有同格版本比對）。只有最新那次嘗試的結果會改「自動存檔失敗」提示；組存檔丟錯也落到提示。
  // 跨對話 global 跟著存（D29），所有存檔共用、重新整理不丟。
  const writesRef = useRef<Promise<unknown>>(Promise.resolve());
  const attemptRef = useRef(0);
  /** 寫一次；回 true＝這一筆寫成了（沒有存檔庫也算，存不了就不擋玩）。 */
  const writeSave = useCallback((): Promise<boolean> => {
    if (!saves) return Promise.resolve(true);
    const hadSlot = slotRef.current;
    slotRef.current = true;
    const attempt = (attemptRef.current += 1);
    const settle = (failed: boolean) => {
      // 第一格沒寫成：下次開口再試著佔格
      if (failed && !hadSlot) slotRef.current = false;
      if (attemptRef.current === attempt) setSaveFailed(failed);
      return !failed;
    };
    // global 先預約再取樣：版本與寫入資格（當時載入過沒有）都在這一刻定，別桌晚到的舊內容蓋不掉這份較新的
    const writeGlobal = saves.reserveGlobalWrite();
    let save: WebSave;
    let global: VariableMap;
    try {
      save = exportSave();
      // 只存過得了契約的存檔（原 PNG 匯入時已驗過，這裡不重驗那一大段）
      if (!parseWebSave(JSON.stringify({ ...save, card_png: undefined })).ok) throw new Error("web save off contract");
      global = JSON.parse(JSON.stringify(GLOBAL_VARIABLES)) as VariableMap;
    } catch {
      return Promise.resolve(settle(true));
    }
    const meta = { id: saveId, title: game.card.text.name, updatedAt: Date.now(), messageCount: save.messages.length };
    const result = writesRef.current
      .then(() => saves.put(meta, save))
      .then(() => writeGlobal(global))
      .then(
        () => settle(false),
        () => settle(true),
      );
    writesRef.current = result;
    return result;
  }, [saves, exportSave, game, saveId]);
  writeSaveRef.current = writeSave;

  const playerSpoke = entries.some((entry) => entry.role === "user");
  useEffect(() => {
    if (busy || (!slotRef.current && !playerSpoke)) return;
    void writeSave();
  }, [busy, entries, playerSpoke, writeSave]);

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
    exportSave,
    exportStChat,
    saveFailed,
  };
}
