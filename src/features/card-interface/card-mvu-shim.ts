// 卡片介面沙盒的 MVU（MagVarUpdate）讀變數墊片：把 app 的狀態樹轉成 MVU 的 stat_data，讓 MVU 前端卡
// 畫得出值。行為對照 MVU 與酒館助手（JS-Slash-Runner）的型別規格與實作行為，只當規格書讀、不抄碼
// （見 .ai/plans/card-mvu-shim.md）。本包只讀；寫入類函式（setMvuVariable、replaceVariables…）不定義。
import { type StateNode, type TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { scriptLiteral } from "./card-chat-shim";

/** MVU 一樓的變數表 */
export interface MvuData {
  stat_data: Record<string, unknown>;
  display_data: Record<string, unknown>;
  delta_data: Record<string, unknown>;
}

/** 交給沙盒的 MVU 快照：每樓資料去重成 states，floorState 依樓號對回 */
export interface CardMvu {
  /** 產生目前殼的那一樓 */
  currentId: number;
  /** 最後一則非 system 樓；酒館助手 message 層的 'latest' 指它。-1＝沒有 */
  latestId: number;
  states: MvuData[];
  floorState: number[];
}

export type StateTree = Record<string, StateNode>;

/** MVU 在 AI 樓尾補的狀態欄占位（MagVarUpdate update_variables.ts 的寫法） */
export const MVU_PLACEHOLDER = "<StatusPlaceHolderImpl/>";

const NUMBER = /^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?$/;
const MACRO = /\{\{(user|char)\}\}/gi;
/** MVU 標記可擴充陣列的元素（schema.ts 的 EXTENSIBLE_MARKER） */
const EXTENSIBLE_MARKER = "$__META_EXTENSIBLE__$";
const hasOwn = (object: object, key: string) => Object.prototype.hasOwnProperty.call(object, key);

function finiteDeep(value: unknown): boolean {
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(finiteDeep);
  if (value !== null && typeof value === "object") return Object.values(value).every(finiteDeep);
  return true;
}

function parseNumber(text: string): number | null {
  if (!NUMBER.test(text)) return null;
  const value = Number(text);
  return Number.isFinite(value) ? value : null;
}

function parseJson(text: string): unknown {
  if (!text.startsWith("[") && !text.startsWith("{")) return undefined;
  try {
    const value: unknown = JSON.parse(text);
    // 巢狀裡任一非有限數字（1e999 → Infinity）深拷貝會變 null：整個葉值保留原字串
    return finiteDeep(value) ? value : undefined;
  } catch {
    return undefined;
  }
}

/**
 * 狀態樹的葉子一律是字串（匯入時引號已剝），照 YAML 純量慣例收窄還原型別。
 * type＝重構記下的原卡欄位型別（number／bool／list），有就照它，不符就保留字串。
 */
export function restoreLeaf(text: string, type?: string): unknown {
  if (type === "number") return parseNumber(text) ?? text;
  if (type === "bool") return text === "true" ? true : text === "false" ? false : text;
  if (type === "list") {
    const value = parseJson(text);
    return Array.isArray(value) ? value : text;
  }
  const number = parseNumber(text);
  if (number !== null) return number;
  if (text === "true") return true;
  if (text === "false") return false;
  if (text === "null") return null;
  const json = parseJson(text);
  return json === undefined ? text : json;
}

interface Macros {
  user: string;
  char: string | null;
}

/** 一次掃完 {{user}}／{{char}}：替換值用回呼交字面，名字裡的 `$&` 不會被當成替換語法，也不會被二次代換 */
function substitute(text: string, macros: Macros): string {
  return text.replace(MACRO, (match, name: string) => {
    if (name.toLowerCase() === "user") return macros.user;
    return macros.char ?? match;
  });
}

/** 資料鍵一律存成自有屬性：`__proto__` 這類鍵用一般指定會改到原型而消失 */
function setOwn(target: Record<string, unknown>, key: string, value: unknown): void {
  Object.defineProperty(target, key, { value, enumerable: true, writable: true, configurable: true });
}

/** MVU 的 metadata 載體：陣列裡 `$arrayMeta: true` 且帶 `$meta` 的元素 */
function isArrayMetaCarrier(value: unknown): boolean {
  return (
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    (value as Record<string, unknown>).$arrayMeta === true &&
    hasOwn(value, "$meta")
  );
}

/**
 * 整份只清一次：MVU 初始化時 cleanUpMetadata 移除物件的 `$meta`、陣列裡的 EXTENSIBLE_MARKER 與
 * metadata 載體元素；巨集在解析 initvar 前整段代換，所以字串值與鍵都換。
 */
function cleanValue(value: unknown, macros: Macros): unknown {
  if (typeof value === "string") return substitute(value, macros);
  if (Array.isArray(value)) {
    return value
      .filter((item) => item !== EXTENSIBLE_MARKER && !isArrayMetaCarrier(item))
      .map((item) => cleanValue(item, macros));
  }
  if (value !== null && typeof value === "object") return cleanObject(value as Record<string, unknown>, macros);
  return value;
}

/** 鍵代換後撞到同層別的鍵（原本就叫那個名字，或另一個也換成同名）時，這個鍵保留原字面，不覆蓋別人 */
function cleanObject(source: Record<string, unknown>, macros: Macros): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  const keys = Object.keys(source);
  const literal = new Set(keys);
  for (const key of keys) {
    if (key === "$meta") continue;
    const renamed = substitute(key, macros);
    const target = renamed !== key && (literal.has(renamed) || hasOwn(result, renamed)) ? key : renamed;
    if (target !== renamed) console.warn("[table-tavern] 變數鍵代換後撞名，保留原字面", key);
    setOwn(result, target, cleanValue(source[key], macros));
  }
  return result;
}

/** 狀態樹 → 還原型別後的原始結構（還沒清 metadata、沒代換巨集） */
function restoreTree(tree: StateTree, path: string[], valueTypes: Record<string, string>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const key of Object.keys(tree)) {
    const node = tree[key];
    const here = [...path, key];
    const dotted = here.join(".");
    const type = hasOwn(valueTypes, dotted) ? valueTypes[dotted] : undefined;
    setOwn(result, key, typeof node === "string" ? restoreLeaf(node, type) : restoreTree(node, here, valueTypes));
  }
  return result;
}

/** 一棵狀態樹 → MVU 一樓的變數表。display_data 給 stat_data 的拷貝；app 不存舊值，delta_data 為空 */
export function buildMvuData(
  tree: StateTree,
  valueTypes: Record<string, string>,
  macros: Macros,
): MvuData {
  const stat = cleanValue(restoreTree(tree, [], valueTypes), macros) as Record<string, unknown>;
  return { stat_data: stat, display_data: JSON.parse(JSON.stringify(stat)) as Record<string, unknown>, delta_data: {} };
}

/**
 * 會改狀態的樓：只有開場（post_opening）與 GM 回覆（gm_narrate）會套用狀態更新，兩者都落成 speaker_id 空字串
 * 的 narration 事件；中止或截斷的 GM 回覆也是同一形狀，一併算進來。玩家句、角色台詞、系統事件落檔時
 * 只是蓋上當下快照。
 */
export function changesState(event: TranscriptEvent): boolean {
  return event.kind === "narration" && event.speaker_id === "";
}

/**
 * 每樓的狀態來源。events＝本場的樓（chatEvents 排除 gm_only 後，與公開樓號同一套索引）。
 * - 該樓之後沒有會改狀態的樓 → "live"：用目前狀態（含面板手動改值；前端事件快照不會跟著手改更新）。
 * - 否則 → 該樓之前（含）最近一則快照的樓號；一則都沒有 → "empty"。
 */
export function floorSources(events: TranscriptEvent[]): ("live" | "empty" | number)[] {
  let lastChange = -1;
  events.forEach((event, index) => {
    if (changesState(event)) lastChange = index;
  });
  let snapshot: number | "empty" = "empty";
  return events.map((event, index) => {
    if (event.state?.tree !== undefined) snapshot = index;
    return index >= lastChange ? "live" : snapshot;
  });
}

/**
 * 組沙盒要的 MVU 快照。events 為空（空桌，開場白當第 0 樓）時只有一樓、用目前狀態。
 * roles＝每樓角色（算 latestId 用），長度與樓數相同。
 */
export function buildCardMvu(input: {
  events: TranscriptEvent[];
  roles: string[];
  currentId: number;
  liveTree: StateTree;
  valueTypes: Record<string, string>;
  macros: Macros;
}): CardMvu {
  const { events, liveTree, valueTypes, macros } = input;
  const sources: ("live" | "empty" | number)[] = events.length === 0 ? ["live"] : floorSources(events);
  const states: MvuData[] = [];
  const seen = new Map<string, number>();
  const bySource = new Map<string, number>();
  const floorState = sources.map((source) => {
    const key = String(source);
    const cached = bySource.get(key);
    if (cached !== undefined) return cached;
    const tree =
      source === "live" ? liveTree : source === "empty" ? {} : ((events[source].state?.tree ?? {}) as StateTree);
    const data = buildMvuData(tree, valueTypes, macros);
    const text = JSON.stringify(data);
    let index = seen.get(text);
    if (index === undefined) {
      index = states.length;
      states.push(data);
      seen.set(text, index);
    }
    bySource.set(key, index);
    return index;
  });
  let latestId = -1;
  input.roles.forEach((role, index) => {
    if (role !== "system") latestId = index;
  });
  return { currentId: input.currentId, latestId, states, floorState };
}

/** 這一樓的 stat_data 有沒有東西（MVU 只在已有 stat_data 時補占位） */
export function hasStatData(mvu: CardMvu, floor: number): boolean {
  const data = mvu.states[mvu.floorState[floor]];
  return data !== undefined && Object.keys(data.stat_data).length > 0;
}

/**
 * MVU 處理完一則收到的 AI 樓（非玩家）、已有 stat_data、內容不短於 5 字且還沒有占位時，在樓尾補
 * `\n\n<StatusPlaceHolderImpl/>`。只有 AI 回覆會觸發這段處理，app 的系統事件不算。開場白（post_opening
 * 寫的那則，事件帶 opening）由 initvar 初始化、不經這段處理，不補；沒有開場、第一樓就是 GM 回覆時照補。
 */
export function withMvuPlaceholder(message: string, opening: boolean, role: string, hasStat: boolean): string {
  if (opening || role !== "assistant" || !hasStat) return message;
  if (message.length < 5 || message.includes(MVU_PLACEHOLDER)) return message;
  return `${message}\n\n${MVU_PLACEHOLDER}`;
}

/**
 * 沙盒裡的 MVU 讀變數函式與酒館助手的事件、共享全域函式。排在讀訊息墊片之後、橋接墊片之前：真 parent
 * 先收進閉包，之後只認它推來、token 相符、形狀正確的快照。快照以 JSON.parse 載入（`__proto__` 鍵照樣是
 * 自有屬性），內部表一律用 Map。推送排進佇列逐一處理：本樓資料變了才依序等 STARTED → SINGLE（逐葉）→ ENDED
 * 的監聽器跑完，再處理下一筆。
 */
export function buildMvuShimSource(mvu: CardMvu, token: string): string {
  return `
(function () {
  var parentRef = window.parent;
  var TOKEN = ${scriptLiteral(token)};
  var store = JSON.parse(${scriptLiteral(JSON.stringify(mvu))});
  var EVENTS = {
    VARIABLE_INITIALIZED: "mag_variable_initialized",
    VARIABLE_UPDATE_STARTED: "mag_variable_update_started",
    COMMAND_PARSED: "mag_command_parsed",
    VARIABLE_UPDATE_ENDED: "mag_variable_update_ended",
    BEFORE_MESSAGE_UPDATE: "mag_before_message_update",
    SINGLE_VARIABLE_UPDATED: "mag_variable_updated"
  };
  var hasOwn = Object.prototype.hasOwnProperty;
  // 事件名 → [{ fn, once }]；Map 不吃原型鍵（"constructor"、"__proto__" 也只是普通事件名）
  var listeners = new Map();

  function copy(value) {
    return value === undefined ? undefined : JSON.parse(JSON.stringify(value));
  }
  function isPlainObject(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }
  function dataAt(id) {
    return store.states[store.floorState[id]];
  }
  function report(error) {
    console.error("[table-tavern] 事件監聽器出錯", error);
  }

  // 每筆註冊是一個物件，移除時標成失效：派送途中被移除的監聽器，輪到它時就不再呼叫
  function retire(list, keep) {
    return list.filter(function (entry) {
      if (keep(entry)) return true;
      entry.active = false;
      return false;
    });
  }
  function removeListener(name, fn) {
    var list = listeners.get(name);
    if (!list) return;
    listeners.set(
      name,
      retire(list, function (entry) {
        return entry.fn !== fn;
      })
    );
  }
  function addListener(name, fn, once, first) {
    if (typeof fn !== "function") return { stop: function () {} };
    var list = listeners.get(name) || [];
    var exists = list.some(function (entry) {
      return entry.fn === fn;
    });
    if (!exists) {
      var entry = { fn: fn, once: once, active: true };
      listeners.set(name, first ? [entry].concat(list) : list.concat([entry]));
    }
    return {
      stop: function () {
        removeListener(name, fn);
      }
    };
  }
  // 調整順序只搬位置，原註冊的 once 設定跟著走；還沒註冊就新註冊在最前／最後
  function moveListener(name, fn, first) {
    var list = listeners.get(name) || [];
    var entry = list.find(function (item) {
      return item.fn === fn;
    });
    if (!entry) return addListener(name, fn, false, first);
    var rest = list.filter(function (item) {
      return item !== entry;
    });
    listeners.set(name, first ? [entry].concat(rest) : rest.concat([entry]));
    return {
      stop: function () {
        removeListener(name, fn);
      }
    };
  }
  // 酒館的事件：監聽器依序一個等完再呼叫下一個；單支拋錯或拒絕記錄後照樣通知其他支。
  // 參數原樣傳（可以是函式、循環參照）；isolate 只給宿主 MVU 推送用，每支拿自己的快照拷貝。
  async function emit(name, args, isolate) {
    var list = (listeners.get(name) || []).slice();
    for (var i = 0; i < list.length; i++) {
      var entry = list[i];
      if (!entry.active) continue;
      if (entry.once) removeListener(name, entry.fn);
      try {
        await entry.fn.apply(null, isolate ? args.map(copy) : args);
      } catch (error) {
        report(error);
      }
    }
  }
  window.eventOn = function (name, fn) {
    return addListener(name, fn, false);
  };
  window.eventOnce = function (name, fn) {
    return addListener(name, fn, true);
  };
  window.eventMakeLast = function (name, fn) {
    return moveListener(name, fn, false);
  };
  window.eventMakeFirst = function (name, fn) {
    return moveListener(name, fn, true);
  };
  window.eventRemoveListener = function (name, fn) {
    removeListener(name, fn);
  };
  window.eventClearEvent = function (name) {
    retire(listeners.get(name) || [], function () {
      return false;
    });
    listeners.delete(name);
  };
  window.eventClearListener = function (fn) {
    listeners.forEach(function (_list, name) {
      removeListener(name, fn);
    });
  };
  window.eventClearAll = function () {
    listeners.forEach(function (list) {
      retire(list, function () {
        return false;
      });
    });
    listeners.clear();
  };
  window.eventEmit = function (name) {
    return emit(name, Array.prototype.slice.call(arguments, 1), false);
  };

  // 共享全域（酒館助手 global.ts）：上游就是 _.has／_.set，路徑語意（括號、索引）照 lodash；沙盒最前面
  // 已內嵌 lodash，這裡在卡片腳本有機會換掉 window._ 之前先收好。initializeGlobal 設值後發
  // global_<名稱>_initialized，waitGlobalInitialized 等的就是這個事件。兩者都不回傳值。
  var lodash = window._;
  window.initializeGlobal = function (name, value) {
    lodash.set(window, name, value);
    void emit("global_" + name + "_initialized", [], false);
  };
  window.waitGlobalInitialized = function (name) {
    if (lodash.has(window, name)) return Promise.resolve();
    return new Promise(function (resolve) {
      addListener("global_" + name + "_initialized", function () {
        resolve();
      }, true);
    });
  };

  // 酒館助手 getAllVariables：本樓的變數表（app 只有 MVU 這層，每樓存的是完整快照）
  window.getAllVariables = function () {
    return copy(dataAt(store.currentId));
  };

  function resolveFloor(option) {
    var count = store.floorState.length;
    var id = option.message_id === undefined ? "latest" : option.message_id;
    if (id === "latest") {
      if (store.latestId < 0) throw new Error("getMvuData: 沒有非 system 樓");
      return store.latestId;
    }
    if (typeof id !== "number" || !Number.isInteger(id) || id < -count || id >= count) {
      throw new Error("getMvuData: message_id 超出範圍 " + String(id));
    }
    return id < 0 ? count + id : id;
  }

  var Mvu = {
    events: EVENTS,
    getMvuData: function (option) {
      var type = option && option.type;
      if (type !== "message") {
        // 非 message 層（chat／character／global…）照酒館的做法在包 2 補；在那之前明確拋錯，不假裝成功
        throw new Error("getMvuData: 尚未支援 " + String(type) + " 層變數");
      }
      return copy(dataAt(resolveFloor(option)));
    },
    getMvuVariable: function (data, path, option) {
      var options = option || {};
      var category = options.category === undefined ? "stat" : options.category;
      var source = data && hasOwn.call(data, category + "_data") ? data[category + "_data"] : undefined;
      var value = lodash.get(source, path);
      if (Array.isArray(value) && value.length === 2 && typeof value[1] === "string") return value[0];
      return value === undefined ? options.default_value : value;
    }
  };
  window.initializeGlobal("Mvu", Mvu);

  function validMvu(value) {
    if (!isPlainObject(value) || !Array.isArray(value.states) || !Array.isArray(value.floorState)) return false;
    for (var i = 0; i < value.states.length; i++) {
      var state = value.states[i];
      if (!isPlainObject(state) || !isPlainObject(state.stat_data)) return false;
    }
    for (var j = 0; j < value.floorState.length; j++) {
      var index = value.floorState[j];
      if (!Number.isInteger(index) || index < 0 || index >= value.states.length) return false;
    }
    var count = value.floorState.length;
    if (!Number.isInteger(value.currentId) || value.currentId < 0 || value.currentId >= count) return false;
    return Number.isInteger(value.latestId) && value.latestId >= -1 && value.latestId < count;
  }

  function diffLeaves(before, after, prefix, out) {
    var keys = [];
    var seen = new Set();
    [before, after].forEach(function (side) {
      if (!isPlainObject(side)) return;
      Object.keys(side).forEach(function (key) {
        if (!seen.has(key)) {
          seen.add(key);
          keys.push(key);
        }
      });
    });
    keys.forEach(function (key) {
      var path = prefix ? prefix + "." + key : key;
      var oldValue = isPlainObject(before) && hasOwn.call(before, key) ? before[key] : undefined;
      var newValue = isPlainObject(after) && hasOwn.call(after, key) ? after[key] : undefined;
      if (isPlainObject(oldValue) && isPlainObject(newValue)) {
        diffLeaves(oldValue, newValue, path, out);
      } else if (JSON.stringify(oldValue) !== JSON.stringify(newValue)) {
        out.push([path, oldValue, newValue]);
      }
    });
    return out;
  }

  async function applyPush(next) {
    var before = dataAt(store.currentId);
    var after = next.states[next.floorState[next.currentId]];
    // app 的推送規則：本樓資料沒變就只換快照、不發事件（onLoad 補推等時機是 app 自己的，照發會重畫兩次）
    if (JSON.stringify(before) === JSON.stringify(after)) {
      store = next;
      return;
    }
    await emit(EVENTS.VARIABLE_UPDATE_STARTED, [before], true);
    store = next;
    var changes = diffLeaves(before.stat_data, after.stat_data, "", []);
    for (var i = 0; i < changes.length; i++) {
      await emit(EVENTS.SINGLE_VARIABLE_UPDATED, [after.stat_data, changes[i][0], changes[i][1], changes[i][2]], true);
    }
    await emit(EVENTS.VARIABLE_UPDATE_ENDED, [after, before], true);
  }

  var queue = Promise.resolve();
  window.addEventListener("message", function (event) {
    if (event.source !== parentRef) return;
    var data = event.data;
    if (!data || data.source !== "table-tavern-host" || data.kind !== "chat" || data.token !== TOKEN) return;
    if (!validMvu(data.mvu)) return;
    var next = copy(data.mvu);
    queue = queue.then(function () {
      return applyPush(next);
    }).then(undefined, report);
  });
})();
`;
}
