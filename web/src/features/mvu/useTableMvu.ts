// 一桌的卡片變數：載 MVU 的卡（判定同桌面版卡片介面）開局照 MVU 初始化、每則 AI 回覆照 MVU 更新變數，
// 酒館助手類巨集在送模前與顯示時代換；卡片 iframe 讀寫各層。chat、global 兩層就是 ST 的聊天變數。
// 執行期（lodash、yaml…）只有載 MVU 或用到類巨集的卡才載。卡上有註冊 schema 的腳本（zod 卡）另開隔離引擎。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CardMvu } from "@desktop/features/card-interface/mvu/card-mvu-shim";
import { parseEvalRequest } from "@desktop/features/card-interface/mvu/card-mvu-eval-host";
import type { WebSaveMvu } from "@desktop/shared/contracts/web-save/web-save";
import { loadsMvu, type PlayCard } from "../cards/play-card";
import type { ChatEntry } from "../chat/chat-turn";
import type { ChatMessage } from "../openrouter/stream-chat";
import type { ChatVariables } from "../sillytavern/variables";
import { createEvaluator, type Evaluator } from "./evaluate";
import type { MvuRuntime } from "./runtime";
import type { ZodRunner } from "./zod/frame-host";
import { findSchemaScript } from "./zod/schema-script";
import { t } from "../../i18n";
import {
  cardMvuBase,
  cardWrite,
  CHARACTER_ID,
  EMPTY_LAYERS,
  isMvuData,
  latestMessageVars,
  layerTables,
  Revisions,
  withoutPlaceholder,
  type ExtraLayers,
  type VarsTable,
} from "./tables";

type RuntimeModule = typeof import("./runtime");

/** 存檔裡帶回來的 MVU（chat、global 兩層另外走 ST 變數） */
export type SavedMvu = Omit<WebSaveMvu, "layers"> & { layers: Omit<WebSaveMvu["layers"], "chat" | "global"> };

export interface TableMvuInput {
  card: PlayCard;
  userName: string;
  variables: ChatVariables;
  /** 接著玩時存檔帶回來的；新開的桌是 undefined */
  saved: SavedMvu | null | undefined;
  /** 接著玩：逐字稿是存檔的，不重新開局（除非存檔裡一張有效表都沒有） */
  resumed: boolean;
  entriesRef: { current: ChatEntry[] };
  setEntries: (update: (previous: ChatEntry[]) => ChatEntry[]) => void;
  /** ST 巨集代換（initvar 解析前的 substitudeMacros） */
  substitute: (text: string) => string;
}

export interface TableMvu {
  /** 這桌載 MVU（每則回覆更新變數、iframe 有 MVU 函式） */
  active: boolean;
  /** 送模前等：執行期載好、開局初始化做完 */
  ready: () => Promise<void>;
  /** 新回覆落地前：MVU 處理最後一則 */
  afterReply: (entries: ChatEntry[]) => Promise<ChatEntry[]>;
  /** 組提示時：zod 桌上一則回覆被擋下的回饋（一則 system 的內容）；沒有回 undefined */
  feedback: (entries: ChatEntry[]) => string | undefined;
  /** 送模前：類巨集代換（讀這一發的變數副本）、MVU 拿掉占位 */
  prepareMessages: (messages: ChatMessage[], entries: ChatEntry[], scopes: ChatVariables) => ChatMessage[];
  /** 顯示時的類巨集代換 */
  display: (text: string, entries: ChatEntry[]) => string;
  /** 存檔的 mvu（chat、global 由呼叫端放）；null＝這桌沒有 MVU 也沒帶過 */
  forSave: () => SavedMvu | null;
  /** 交給 iframe 的快照（currentId 由各支換）；null＝沒有 MVU */
  frontend: CardMvu | null;
  write: (data: Record<string, unknown>, reply: (message: Record<string, unknown>) => void) => void;
  evaluate: (data: Record<string, unknown>, reply: (message: Record<string, unknown>) => void) => void;
  /** 開局初始化失敗（哪一條 initvar 讀不懂） */
  initError: string | null;
  /** zod 桌：初始值沒過卡片的變數規則，照原值開局 */
  zodInitInvalid: boolean;
  /** zod 桌整次失敗（載入規則、逾時、出錯），下次成功就清掉：init＝開局失敗、還沒有卡片變數（玩家下一次送出前重試）；
   *  opening＝開場白的指令沒套用、變數停在初始值；update＝最近一則回覆沒寫變數、停在上一則 */
  zodFailure: ZodFailure;
}

export type ZodFailure = "init" | "opening" | "update" | null;

const isObject = (value: unknown): value is VarsTable => typeof value === "object" && value !== null && !Array.isArray(value);

/** 新開的桌：character 層取卡上酒館助手的角色變數、script 層取各腳本自己的資料（酒館助手存在卡裡的位置） */
function cardLayers(card: PlayCard): ExtraLayers {
  const shell = card.shell as VarsTable;
  const data = isObject(shell?.data) ? shell.data : shell;
  const helper = isObject(data?.extensions) && isObject(data.extensions.tavern_helper) ? data.extensions.tavern_helper : null;
  const script: Record<string, VarsTable> = {};
  for (const item of Array.isArray(helper?.scripts) ? helper.scripts : []) {
    if (isObject(item) && typeof item.id === "string" && item.id !== "" && isObject(item.data) && Object.keys(item.data).length > 0) {
      script[item.id] = structuredClone(item.data);
    }
  }
  return {
    ...EMPTY_LAYERS,
    character: isObject(helper?.variables) ? structuredClone(helper.variables) : {},
    script,
  };
}

/** 整份換掉一個 ST 變數範圍的內容（原地改：global 是所有桌共用的同一個物件） */
function replaceValues(into: Record<string, unknown>, table: VarsTable): void {
  for (const key of Object.keys(into)) delete into[key];
  Object.assign(into, table);
}

export function useTableMvu(input: TableMvuInput): TableMvu {
  const { card, userName, variables, saved, resumed, entriesRef, setEntries } = input;
  const active = loadsMvu(card);
  const schema = useMemo(() => {
    if (!active) return null;
    const shell = card.shell as VarsTable;
    return findSchemaScript(isObject(shell?.data) ? shell.data : shell);
  }, [active, card.shell]);
  const needsRuntime = useMemo(() => active || JSON.stringify(card.shell).includes("_variable::"), [active, card.shell]);
  const macros = useMemo(() => saved?.macros ?? { user: userName, char: card.text.name || null }, [saved, userName, card.text.name]);
  const seedRef = useRef<VarsTable | null>(saved?.seed ?? null);
  const [layers, setLayers] = useState<ExtraLayers>(() => (saved ? { ...saved.layers } : resumed ? EMPTY_LAYERS : cardLayers(card)));
  const layersRef = useRef(layers);
  layersRef.current = layers;
  // 執行期載好了沒（state：顯示要跟著重算）；模組與實例另存 ref，回呼裡同步拿得到
  const [runtimeLoaded, setRuntimeLoaded] = useState(false);
  const runtimeModule = useRef<RuntimeModule | null>(null);
  const runtimeRef = useRef<MvuRuntime | null>(null);
  // 變數一動（卡片寫入）就換新快照推給 iframe
  const [version, setVersion] = useState(0);
  const [initError, setInitError] = useState<string | null>(null);
  const [zodInitInvalid, setZodInitInvalid] = useState(false);
  const [zodFailure, setZodFailure] = useState<ZodFailure>(null);
  const revisions = useMemo(() => new Revisions(), []);
  // 值解析 Worker：跟著這桌掛載（StrictMode 重掛也是新的一支）；執行期拿的是轉交的這一層
  const evaluatorRef = useRef<Evaluator | null>(null);
  useEffect(() => {
    const created = createEvaluator();
    evaluatorRef.current = created;
    return () => {
      created.dispose();
      if (evaluatorRef.current === created) evaluatorRef.current = null;
    };
  }, []);
  const evaluator = useMemo<Evaluator>(
    () => ({
      run: (op, text) => evaluatorRef.current?.run(op, text) ?? Promise.resolve({ ok: false, error: "closed" }),
      dispose: () => {},
    }),
    [],
  );
  const inputRef = useRef(input);
  inputRef.current = input;
  // zod 桌的隔離引擎：用到才建（執行期那一包裡），跟著這桌卸載回收；回收後再用就建新的
  const zodRef = useRef<ZodRunner | null>(null);
  useEffect(
    () => () => {
      zodRef.current?.dispose();
      zodRef.current = null;
    },
    [],
  );

  // 開局（照 MVU：接著玩時只在存檔裡一張有效表都沒有才初始化）。zod 桌開局整次失敗就留到玩家下一次送出、組提示之前再試
  const initRetryRef = useRef(false);
  const initialize = useCallback(
    async (module: RuntimeModule, loaded: MvuRuntime) => {
      initRetryRef.current = false;
      if (!active || seedRef.current !== null || entriesRef.current.some((entry) => isMvuData(entry.vars))) return;
      const first = entriesRef.current[0];
      const opening = !resumed && first?.opening ? first : null;
      let outcome: Awaited<ReturnType<RuntimeModule["initializeVariables"]>>;
      try {
        outcome = await module.initializeVariables(loaded, card, opening?.text ?? null, inputRef.current.substitute);
      } catch (error) {
        if (!(error instanceof module.ZodRunError)) throw error;
        console.error("[table-tavern] 卡片變數規則執行失敗", error);
        initRetryRef.current = true;
        setZodFailure("init");
        return;
      }
      if (!outcome.ok) {
        setInitError(outcome.comment);
        return;
      }
      seedRef.current = outcome.table;
      if (outcome.zodInitInvalid) setZodInitInvalid(true);
      setZodFailure(outcome.openingFailed === true ? "opening" : null);
      // 開場那一樓（接著玩則是最後一樓）掛上這張表；zod 桌開場白更新失敗時那一樓不寫
      const targetId = opening?.id ?? entriesRef.current[entriesRef.current.length - 1]?.id;
      const vars = opening ? outcome.opening : outcome.table;
      if (targetId !== undefined && vars !== null) {
        // 開場白指令被擋下的回饋掛在開場那一樓，第一輪送 GM
        const notes = opening && outcome.openingNotes?.length ? { mvuNotes: outcome.openingNotes } : {};
        const attach = (entry: ChatEntry) => (entry.id === targetId ? { ...entry, vars, ...notes } : entry);
        entriesRef.current = entriesRef.current.map(attach);
        setEntries((previous) => previous.map(attach));
      }
    },
    [active, entriesRef, resumed, card, setEntries],
  );

  // 載執行期，載 MVU 的卡再開局。一桌只載一次，開局用的是那時的卡與逐字稿
  const loadRef = useRef<Promise<void> | null>(null);
  const load = useCallback(() => {
    loadRef.current ??= !needsRuntime
      ? Promise.resolve()
      : import("./runtime").then(async (module) => {
          runtimeModule.current = module;
          const zod =
            schema === null
              ? null
              : () => (zodRef.current ??= module.createZodRunner({ schema }));
          const loaded = module.createRuntime(evaluator, macros, zod);
          runtimeRef.current = loaded;
          await initialize(module, loaded);
          setRuntimeLoaded(true);
        });
    return loadRef.current;
  }, [needsRuntime, evaluator, macros, schema, initialize]);
  // 送模前：第一次是載入與開局；之後若 zod 桌開局整次失敗過，就在這裡（組提示之前）重試一次，同一輪的 afterReply 不再試
  const retryRef = useRef<Promise<void> | null>(null);
  const ready = useCallback(() => {
    if (loadRef.current === null) return load();
    return load().then(() => {
      const module = runtimeModule.current;
      const loaded = runtimeRef.current;
      if (!initRetryRef.current || !module || !loaded) return;
      retryRef.current ??= initialize(module, loaded).finally(() => {
        retryRef.current = null;
      });
      return retryRef.current;
    });
  }, [load, initialize]);
  useEffect(() => {
    load().catch((error: unknown) => console.error("[table-tavern] 卡片變數執行期載入失敗", error));
  }, [load]);

  // 類巨集讀的變數：送模時是該發組提示用的副本（setvar 的副作用照這一發，換模第二發不讀第一發提交的值）
  const reader = useCallback(
    (entries: ChatEntry[], scopes: ChatVariables = variables) => (layer: string) => {
      switch (layer) {
        case "message":
          return latestMessageVars(entries);
        case "chat":
          return scopes.local.values;
        case "global":
          return scopes.global.values;
        case "character":
          return layersRef.current.character;
        default:
          return layersRef.current.preset;
      }
    },
    [variables],
  );

  const prepareMessages = useCallback(
    (messages: ChatMessage[], entries: ChatEntry[], scopes: ChatVariables) => {
      const module = runtimeModule.current;
      if (!module && !active) return messages;
      const read = reader(entries, scopes);
      return messages.map((message) => {
        let content = module ? module.replaceMacroLike(message.content, read) : message.content;
        if (active) content = withoutPlaceholder(content);
        return content === message.content ? message : { ...message, content };
      });
    },
    [active, reader],
  );

  // zod 桌：這批逐字稿裡最後一則 AI 回覆的變數指令被擋下了，回饋給 GM 的那則 system（同桌面狀態區段那段，標題跟著語系）；
  // 由 composePrompt 當固定段落算進預算
  const feedback = useCallback(
    (entries: ChatEntry[]) => {
      if (schema === null) return undefined;
      const notes = [...entries].reverse().find((entry) => entry.role === "char")?.mvuNotes;
      return notes?.length ? [t("mvuNotesHeading"), ...notes].join("\n") : undefined;
    },
    [schema],
  );

  const display = useCallback(
    (text: string, entries: ChatEntry[]) => (runtimeLoaded && runtimeModule.current ? runtimeModule.current.replaceMacroLike(text, reader(entries)) : text),
    [runtimeLoaded, reader],
  );

  const afterReply = useCallback(
    async (entries: ChatEntry[]) => {
      if (!active) return entries;
      await load();
      const module = runtimeModule.current;
      const loaded = runtimeRef.current;
      const last = entries[entries.length - 1];
      if (!module || !loaded || !last || last.role !== "char" || last.opening) return entries;
      let updated: ChatEntry | null;
      try {
        updated = await module.processReply(loaded, entries, entries.length - 1, seedRef.current);
      } catch (error) {
        if (!(error instanceof module.ZodRunError)) throw error;
        console.error("[table-tavern] 這則回覆的卡片變數更新失敗", error);
        setZodFailure("update");
        return entries;
      }
      if (updated === null) return entries;
      if (loaded.zod) setZodFailure(null);
      return [...entries.slice(0, -1), updated];
    },
    [active, load],
  );

  const forSave = useCallback((): SavedMvu | null => {
    if (!active && !saved) return null;
    return { macros: active ? macros : (saved?.macros ?? null), seed: seedRef.current, layers: layersRef.current };
  }, [active, saved, macros]);

  const entriesNow = entriesRef.current;
  const frontend = useMemo(
    () => (active ? cardMvuBase(entriesNow, layerTables(variables.local.values, variables.global.values, layers), macros, revisions) : null),
    // version：卡片寫入了 chat／global（原地改的物件，參照不變）
    [active, entriesNow, layers, macros, variables, version, revisions],
  );

  const write = useCallback(
    (data: Record<string, unknown>, reply: (message: Record<string, unknown>) => void) => {
      if (!active) return;
      const settle = cardWrite(data, {
        read: (key) => {
          if (key === "chat") return { found: true, table: variables.local.values };
          if (key === "global") return { found: true, table: variables.global.values };
          if (key === `character:${CHARACTER_ID}`) return { found: true, table: layersRef.current.character };
          if (key === "preset") return { found: true, table: layersRef.current.preset };
          const scoped = /^(script|extension):([\s\S]+)$/.exec(key);
          if (scoped) return { found: true, table: layersRef.current[scoped[1] as "script" | "extension"][scoped[2]] ?? null };
          const entry = entriesRef.current.find((item) => item.id === key);
          return entry === undefined ? { found: false, table: null } : { found: true, table: entry.vars ?? null };
        },
        write: (key, table) => {
          if (key === "chat") {
            replaceValues(variables.local.values, table);
          } else if (key === "global") {
            for (const name of new Set([...Object.keys(variables.global.values), ...Object.keys(table)])) variables.global.written.add(name);
            replaceValues(variables.global.values, table);
          } else if (key === `character:${CHARACTER_ID}` || key === "preset" || /^(script|extension):/.test(key)) {
            const scoped = /^(script|extension):([\s\S]+)$/.exec(key);
            const next: ExtraLayers = scoped
              ? { ...layersRef.current, [scoped[1]]: { ...layersRef.current[scoped[1] as "script" | "extension"], [scoped[2]]: table } }
              : { ...layersRef.current, [key === "preset" ? "preset" : "character"]: table };
            layersRef.current = next;
            setLayers(next);
          } else {
            const next = entriesRef.current.map((entry) => (entry.id === key ? { ...entry, vars: table } : entry));
            entriesRef.current = next;
            setEntries((previous) => previous.map((entry) => (entry.id === key ? { ...entry, vars: table } : entry)));
          }
          setVersion((value) => value + 1);
        },
      }, revisions);
      if (settle !== null) reply(settle);
    },
    [active, variables, entriesRef, setEntries, revisions],
  );

  const evaluate = useCallback(
    (data: Record<string, unknown>, reply: (message: Record<string, unknown>) => void) => {
      const request = active ? parseEvalRequest(data) : null;
      if (request === null) return;
      void evaluator
        .run(request.op, request.text)
        .catch((reason: unknown) => ({ ok: false as const, error: `host-error: ${String(reason)}` }))
        .then((outcome) => reply({ kind: "mvu-eval-result", requestId: request.requestId, ...outcome }));
    },
    [active, evaluator],
  );

  return useMemo(
    () => ({ active, ready, afterReply, feedback, prepareMessages, display, forSave, frontend, write, evaluate, initError, zodInitInvalid, zodFailure }),
    [active, ready, afterReply, feedback, prepareMessages, display, forSave, frontend, write, evaluate, initError, zodInitInvalid, zodFailure],
  );
}
