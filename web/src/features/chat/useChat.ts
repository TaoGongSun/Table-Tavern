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
import { commitVariables, copyVariables, createChatVariables, turnCommits, type VariableMap } from "../sillytavern/variables";
import { parseWebSave, type WebSave } from "@desktop/shared/contracts/web-save/web-save";
import { deleteLast, regenerateBase, replaceLast, resolveTurn, type ChatEntry, type PendingTurn } from "./chat-turn";
import { t } from "../../i18n";
import { explainError } from "./error-text";
import { composePrompt } from "./prompt";
import { messageTokenCounter, textTokenCounter } from "../sillytavern/tokens";
import { tableWorldInfo, worldInfoForSave } from "./world-info-setup";
import { editedText, macroContext, openingText, replyText, userText, type ChatSetup } from "./st-text";
import { useTableMvu, type TableMvu } from "../mvu/useTableMvu";

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

/**
 * 回合收尾時以回合開頭的逐字稿為底：期間卡片介面寫進某一則的變數表（同 id）要留著，不被底稿蓋回舊表。
 */
export function keepLatestVars(entries: ChatEntry[], latest: ChatEntry[]): ChatEntry[] {
  const byId = new Map(latest.map((entry) => [entry.id, entry]));
  return entries.map((entry) => {
    const now = byId.get(entry.id);
    if (now === undefined || now.vars === entry.vars) return entry;
    const { vars: _old, ...rest } = entry;
    return now.vars === undefined ? rest : { ...rest, vars: now.vars };
  });
}

/** 兩份同長的逐字稿每一則掛的是同一張表（卡片寫入一律換新的表物件，同一張＝期間沒寫過） */
export function sameVars(a: ChatEntry[], b: ChatEntry[]): boolean {
  return a.length === b.length && a.every((entry, index) => entry.vars === b[index].vars);
}

/** MVU 處理回覆時底稿被卡片改掉、以新底稿重算的次數上限 */
const MVU_REBASE_TRIES = 3;

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
  /** 卡片介面的按鈕送出一句（不經輸入框；回合進行中就不送） */
  sendText: (text: string) => Promise<void>;
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
  /** 卡片介面存的設定（沙盒 localStorage 的整份快照），跟著存檔走 */
  cardStorage: Record<string, string>;
  /** 卡片變數（MVU、酒館助手變數層） */
  mvu: Pick<TableMvu, "frontend" | "write" | "evaluate" | "display" | "initError">;
  setCardStorage: (entries: Record<string, string>) => void;
}

export function useChat(game: GameSetup, session: OpenRouterSession, saves: SaveStore | null = null): ChatController {
  const [saveId] = useState(() => game.saveId ?? newSaveId());
  const worldInfo = useMemo(() => tableWorldInfo(game.card, (game.resume?.carry ?? EMPTY_CARRY).worldInfo), [game.card, game.resume]);
  const setup = useMemo<ChatSetup>(
    () => ({
      card: game.card,
      userName: game.userName,
      variables: createChatVariables(structuredClone(game.resume?.local ?? {}), GLOBAL_VARIABLES),
      chatId: `web-${saveId}`,
      worldInfo,
    }),
    [game.card, game.userName, game.resume, saveId, worldInfo],
  );
  const [entries, setEntries] = useState<ChatEntry[]>(() => game.resume?.entries ?? openingEntries(setup, game.openingIndex));
  const [saveFailed, setSaveFailed] = useState(false);
  const [cardStorage, setCardStorage] = useState<Record<string, string>>(() => game.resume?.carry.cardStorage ?? {});
  const cardStorageRef = useRef(cardStorage);
  cardStorageRef.current = cardStorage;
  // 上次在回合進行中重新整理（D28）：那一句放回輸入框
  const [input, setInput] = useState(() => (game.resume ? readPendingInput(saveId) : null) ?? "");
  const [busy, setBusy] = useState(false);
  const [streaming, setStreaming] = useState("");
  // 錯誤存成「怎麼說」而不是說好的字：換語系時跟著換
  const [error, setError] = useState<(() => string) | null>(null);
  const [failover, setFailover] = useState<FailoverNotice | null>(null);
  const busyRef = useRef(false);
  const abortRef = useRef<AbortController | null>(null);
  /** 回合世代：卸載時推進，晚到的結果認得出自己已作廢。 */
  const generationRef = useRef(0);
  const entriesRef = useRef(entries);
  entriesRef.current = entries;
  const mvu = useTableMvu({
    card: game.card,
    userName: game.userName,
    variables: setup.variables,
    saved: game.resume?.carry.mvu,
    resumed: game.resume !== undefined,
    entriesRef,
    setEntries,
    substitute: (text) => substituteParams(text, macroContext(setup, entriesRef.current)),
  });
  /** 把這桌現在的樣子寫進存檔庫（下面定義；回合開始時要先用） */
  const writeSaveRef = useRef<() => Promise<boolean>>(async () => true);
  /** 這桌已經佔了一格存檔 */
  const slotRef = useRef(game.resume !== undefined);
  /** 整頁正在離開（重新整理、關分頁）：作廢的回合不收尾，也不再自動存檔，存檔停在上一個完整回合（D28） */
  const leavingRef = useRef(false);
  /**
   * D28 草稿的流水號：每記一次回合中的那一句就推進；回合落地時記下它，等含那一回合的存檔寫成才清草稿（寫成之前
   * 重新整理，存檔還是上一回合，那一句要能放回輸入框）。
   */
  const draftRef = useRef(0);
  const landedDraftRef = useRef(0);

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
  // 重新整理、關分頁（D28）：瀏覽器在 pagehide 之前就中斷請求、腳本還會跑一下，那個失敗結果不能當成回合收尾——
  // 否則草稿被清掉、半截回合被自動存進去。整頁要離開（beforeunload）就比照卸載作廢在途回合、停掉自動存檔；
  // 進往返快取（persisted）的頁面之後還會回來，pagehide 那時不動，回來時（pageshow）恢復自動存檔
  useEffect(() => {
    const leave = () => {
      leavingRef.current = true;
      generationRef.current += 1;
      abortRef.current?.abort();
    };
    const hide = (event: PageTransitionEvent) => {
      if (!event.persisted) leave();
    };
    const show = (event: PageTransitionEvent) => {
      if (event.persisted) leavingRef.current = false;
    };
    window.addEventListener("beforeunload", leave);
    window.addEventListener("pagehide", hide);
    window.addEventListener("pageshow", show);
    return () => {
      window.removeEventListener("beforeunload", leave);
      window.removeEventListener("pagehide", hide);
      window.removeEventListener("pageshow", show);
    };
  }, []);

  /**
   * 一個回合：送出前查今日免費次數 → `begin` 把逐字稿改成這輪的起點（回 null＝不送）→ 選模串流 → 收尾。
   * 第一個 await 之前就備好取消與世代：查詢期間按停止或卸載也算數，逐字稿與輸入框都不動。
   */
  const runTurn = useCallback(
    async (begin: () => { before: ChatEntry[]; pending: PendingTurn } | null, generationType: "normal" | "regenerate") => {
      const apiKey = session.apiKey;
      if (!apiKey || busyRef.current || quotaBlocksSending(session.quota)) return;
      busyRef.current = true;
      // 離開被取消（頁面沒真的換掉）後玩家又開口：恢復自動存檔
      leavingRef.current = false;
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
            setError(() => () => t("autosaveFirstFailed"));
            return;
          }
        }
        // 卡片變數：執行期載好、開局初始化做完才組提示（類巨集要讀第 0 樓的表）
        await mvu.ready().catch(() => {});
        if (cancelled()) return;
        const started = begin();
        if (!started) return;
        const { pending } = started;
        // 回合中的那一句不在存檔裡：另記草稿，重新整理後放回輸入框（D28）
        const draft = (draftRef.current += 1);
        if (pending.kind === "send") rememberPendingInput(saveId, pending.rawInput);
        const input = pending.kind === "send" ? pending.rawInput : "";
        setEntries(started.before);
        // 每支模型照它的上限與 token 估算組一次提示（D23），都從回合開頭的變數副本起算；真正派送的那一次才把
        // 副作用（setvar 等）與第 0 則寫回落地，選模時試組與換模前的那一發不重複提交
        const snapshot = copyVariables(setup.variables);
        // chat／global 的提交紀錄：卡片在回合中任何時候寫過的鍵，每一發（含換模第二發）提交都照 message 表的
        // 規則保住，不被回合開頭的副本蓋回去
        const commits = turnCommits(setup.variables);
        // 世界書觸發狀態同理：每一發都從回合開頭的狀態掃（換模第二發不接第一發落地的狀態）
        const worldInfoAtStart = { entries: worldInfo.entries, state: worldInfo.state };
        const composed = new Map<string, ReturnType<typeof composeFor>>();
        const composeFor = (model: string) => {
          const variables = copyVariables(snapshot);
          const limits = { maxContext: session.pool.contextLength(model), maxResponse: RESERVED_OUTPUT_TOKENS };
          const tokenizer = session.pool.tokenizer(model);
          const countTokens = messageTokenCounter(model, tokenizer);
          const countText = textTokenCounter(model, tokenizer);
          const prompt = composePrompt({ ...setup, variables, worldInfo: worldInfoAtStart }, started.before, {
            generationType,
            input,
            model,
            limits,
            countTokens,
            countText,
          });
          // 酒館助手的類巨集與 MVU 拿掉占位：組好整份提示之後（ST 的 GENERATE_AFTER_DATA）
          return { variables, ...prompt, messages: mvu.prepareMessages(prompt.messages, prompt.entries, variables) };
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
              commitVariables(setup.variables, turn.variables, commits);
              if (turn.worldInfo) worldInfo.state = turn.worldInfo;
              setEntries(keepLatestVars(turn.entries, entriesRef.current));
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
        // 畫面已卸載或整頁要離開：結果不再寫回（草稿留著，接著玩時放回輸入框）
        if (generationRef.current !== generation) return;

        const before = dispatched.turn?.entries ?? started.before;
        const result = resolveTurn(before, pending, outcome, newId, (text) => replyText(setup, before, text));
        let landed = result.entries;
        // 新回覆照 MVU 更新變數（落地前就算好，存檔與畫面拿到的是同一則）
        const reply = landed[landed.length - 1];
        if (reply && reply.role === "char" && !before.some((entry) => entry.id === reply.id)) {
          // 底稿先換成卡片在回合中寫過的最新表，MVU 才從最新的有效表算起；值解析等待期間卡片又寫了前面的表，
          // 這次算出來的就過期了，以新的底稿重算。一直被改就不提交過期的計算，這則回覆不更新變數
          const resolved = landed;
          landed = keepLatestVars(resolved, entriesRef.current);
          for (let tries = 0; tries < MVU_REBASE_TRIES; tries += 1) {
            const base = keepLatestVars(resolved, entriesRef.current);
            const updated = await mvu.afterReply(base);
            if (generationRef.current !== generation) return;
            if (sameVars(base, keepLatestVars(resolved, entriesRef.current))) {
              landed = updated;
              break;
            }
          }
        }
        setEntries(keepLatestVars(landed, entriesRef.current));
        // 回合落地：草稿等含這一回合的存檔寫成才清（沒有存檔庫就沒有可退的存檔，現在就清）
        landedDraftRef.current = draft;
        if (!saves) forgetPendingInput(saveId);
        setStreaming("");
        setFailover(outcome.failover);
        if (result.restoreInput !== null) setInput(result.restoreInput);
        if (isDailyExhausted(outcome)) session.quotaEvent({ type: "daily-exhausted-error" });
        else if (result.error !== null) {
          const display = result.error;
          setError(() => () => explainError(display));
        }
        // 更新今日免費次數（/key 不佔次數）
        void session.refreshQuota();
      } finally {
        if (abortRef.current === controller) abortRef.current = null;
        busyRef.current = false;
        setBusy(false);
      }
    },
    [session, setup, saveId, worldInfo, mvu, saves],
  );

  const submit = useCallback(
    async (text: string, fromInput: boolean) => {
      const raw = text.trim();
      if (!raw) return;
      await runTurn(() => {
        const current = entriesRef.current;
        const userEntry: ChatEntry = { id: newId(), role: "user", text: userText(setup, current, raw), sentAt: Date.now() };
        if (fromInput) setInput("");
        return { before: [...current, userEntry], pending: { kind: "send", userEntry, rawInput: raw } };
      }, "normal");
    },
    [runTurn, setup],
  );
  const send = useCallback(() => submit(input, true), [submit, input]);
  const sendText = useCallback((text: string) => submit(text, false), [submit]);

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
        carry: { ...(game.resume?.carry ?? EMPTY_CARRY), cardStorage: cardStorageRef.current, mvu: mvu.forSave() },
        worldInfo: worldInfoForSave(
          worldInfo,
          (game.resume?.carry ?? EMPTY_CARRY).worldInfo,
          new Set(entriesRef.current.map((entry) => entry.id)),
        ),
        exportedAt: Date.now(),
      }),
    [game, setup, worldInfo, mvu],
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
    // 整頁要離開：存檔停在上一個完整回合（作廢回合的半截不存）
    if (leavingRef.current) return Promise.resolve(false);
    // 這一筆取樣時已落地的回合：寫成了，那一回合的草稿就可以清（之後又開了新回合、記了新草稿就不清）
    const landedDraft = landedDraftRef.current;
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
        () => {
          if (landedDraft !== 0 && landedDraft === draftRef.current) forgetPendingInput(saveId);
          return settle(false);
        },
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
  }, [busy, entries, cardStorage, playerSpoke, writeSave]);

  return {
    setup,
    entries,
    input,
    setInput,
    busy,
    streaming,
    error: error?.() ?? null,
    failover,
    send,
    sendText,
    regenerate,
    canRegenerate: regenerateBase(entries) !== null,
    editLast,
    deleteLast: removeLast,
    stop,
    exportSave,
    exportStChat,
    saveFailed,
    cardStorage,
    setCardStorage,
    mvu,
  };
}
