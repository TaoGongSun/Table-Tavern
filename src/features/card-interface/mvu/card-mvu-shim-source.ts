// 卡片介面沙盒裡的 MVU 與酒館助手變數函式（純 ES5＋async 的原始碼字串）：讀取、寫入、事件、共享全域。
// 行為對照酒館助手（JS-Slash-Runner `src/function/variables.ts`、`event.ts`、`global.ts`）與 MVU
// （MagVarUpdate `src/function/global/index.ts`、`update_variables.ts`），只當規格書讀、不抄碼
// （計畫 .ai/plans/card-mvu-shim.md 第 3、8.5、8.6 節）。
import { scriptLiteral } from "../card-chat-shim";
import { type CardMvu } from "./card-mvu-shim";
import { buildMvuParseSource } from "./card-mvu-parse-source";

/**
 * 排在讀訊息墊片之後、橋接墊片之前：真 parent 先收進閉包，之後只認它推來、token 相符、形狀正確的快照與
 * 結算。快照以 JSON.parse 載入（`__proto__` 鍵照樣是自有屬性），內部表一律用 Map。
 *
 * 寫入（message 層，包 2a）：先改本地值（保持上游同步語意），再把整張表送宿主；宿主確認落檔才結算。
 * 本地值依寫入目標（事件 key）暫存，結算前推來的快照不蓋掉它；被拒時換成宿主推回的權威值並發外部變動事件。
 *
 * parseMessage（包 2c）：流程整段在沙盒跑，值解析與數學式逐條送宿主的專用 Worker（`mvu-eval`／`mvu-eval-result`）。
 *
 * 非 message 層（包 2b）：chat／character／global／preset／script／extension 各是一份「整張表」，沿用同一套
 * 本地值、版本與結算機制，寫入目標 key 是層名（script／extension／character 帶 `:原 ID`），宿主依 key 決定
 * 落檔位置。讀值是同步的，所以面板掛載時宿主把這些層（script／extension 已存在的全部）一併放進快照。
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
  // 寫入目標 key → 還沒結算的本地值與筆數；requestId → 等結算的 Promise
  var local = new Map();
  var pending = new Map();
  var waiting = new Map();
  // 寫入目標 key → 本地值是以哪一版為底（確認或推回權威值後換成那一版）；以及已知比本地值舊的版本：
  // 推來的快照還是這些舊版本就照留本地值，其他版本（確認的那版、或別處寫入的更新版）一律改讀快照
  var bases = new Map();
  var oldRevs = new Map();
  var sequence = 0;

  function copy(value) {
    return value === undefined ? undefined : JSON.parse(JSON.stringify(value));
  }
  function isPlainObject(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }
  function targetAt(id) {
    return store.targets[id] || null;
  }
  function dataAt(id) {
    var target = targetAt(id);
    if (target && local.has(target.key)) return local.get(target.key);
    return store.states[store.floorState[id]];
  }
  // 非 message 層：key 與宿主的寫入目標同一套；快照沒列的層是空表、版本 null（檔案還不存在）
  function layerDoc(key) {
    return hasOwn.call(store.layers, key) ? store.layers[key] : null;
  }
  // 讀取失敗的層（或所在類別）：不能當空表初始化，讀寫一律拋錯
  function layerCheck(name, key) {
    var doc = layerDoc(key);
    var category = /^(script|extension):/.exec(key);
    var block = category ? layerDoc(category[1] + ":") : null;
    var problem = (doc && doc.error) || (block && block.error);
    if (problem) throw new Error(name + ": 變數層讀取失敗（" + problem + "），重開面板重試");
  }
  function layerData(key) {
    if (local.has(key)) return local.get(key);
    var doc = layerDoc(key);
    return doc ? doc.vars : {};
  }
  function layerRev(key) {
    var doc = layerDoc(key);
    return doc ? doc.rev : null;
  }
  function isLayerKey(key) {
    return key === "chat" || key === "global" || key === "preset" || /^(character|script|extension):/.test(key);
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

  // 酒館助手 getAllVariables：全域 → 角色 → 聊天 → 0 樓到本樓各 message 層依序頂層 assign（沒有表的樓
  // 帶出前面樓的值）。
  window.getAllVariables = function () {
    var result = {};
    ["global", "character:" + store.characterId, "chat"].forEach(function (key) {
      layerCheck("getAllVariables", key);
    });
    lodash.assign(result, layerData("global"), layerData("character:" + store.characterId), layerData("chat"));
    for (var i = 0; i <= store.currentId; i++) lodash.assign(result, dataAt(i));
    return copy(result);
  };

  // 解析 option：回 null＝message 層，否則回非 message 層的寫入目標 key。預設層是 chat；
  // script／extension 必須給 ID（酒館助手沒給 script_id 會拋錯）；character 只認目前殼所屬的卡；
  // preset 只有 app 這一份（預設集名稱只認 in_use／app）。
  function resolveLayer(name, option) {
    var normalized = option === undefined ? {} : option;
    if (!isPlainObject(normalized)) throw new Error(name + ": option 必須是物件");
    var type = normalized.type === undefined ? "chat" : normalized.type;
    switch (type) {
      case "message":
        return null;
      case "chat":
      case "global":
        return type;
      case "preset":
        if (normalized.preset_name !== undefined && normalized.preset_name !== "in_use" && normalized.preset_name !== "app") {
          throw new Error(name + ": 預設集不存在 " + String(normalized.preset_name));
        }
        return "preset";
      case "character":
        if (normalized.character_name !== undefined && normalized.character_name !== "current") {
          throw new Error(name + ": 只支援目前這張卡的角色變數");
        }
        if (store.characterId === "") throw new Error(name + ": 這份殼找不到唯一所屬的卡，不能讀寫角色變數");
        return "character:" + store.characterId;
      case "script":
      case "extension":
        var id = normalized[type + "_id"];
        if (typeof id !== "string" || id === "" || Array.from(id).length > 256) {
          throw new Error(name + ": 未指定 " + type + "_id（1～256 字元）");
        }
        return type + ":" + id;
      default:
        throw new Error(name + ": 不支援的變數層 " + String(type));
    }
  }
  function messageOption(option) {
    return option === undefined ? { type: "message" } : option;
  }

  // 讀取的樓號：'latest'（也是不給 message_id 時的預設）＝最後一則非 system 樓；明確給 -1 才是最後一樓
  function readFloor(option) {
    var count = store.floorState.length;
    var id = option.message_id === undefined ? "latest" : option.message_id;
    if (id === "latest") {
      if (store.latestId < 0) throw new Error("getVariables: 沒有非 system 樓");
      return store.latestId;
    }
    if (typeof id !== "number" || !Number.isInteger(id) || id < -count || id >= count) {
      throw new Error("getVariables: message_id 超出範圍 " + String(id));
    }
    return id < 0 ? count + id : id;
  }
  // 寫入的樓號：'latest'／不給＝最後一樓（含 system，照酒館助手 replaceVariables）
  function writeFloor(option) {
    var count = store.floorState.length;
    var id = option.message_id === undefined || option.message_id === "latest" ? -1 : option.message_id;
    if (typeof id !== "number" || !Number.isInteger(id) || id < -count || id >= count) {
      throw new Error("replaceVariables: message_id 超出範圍 " + String(id));
    }
    return id < 0 ? count + id : id;
  }

  function finiteDeep(value, seen) {
    if (typeof value === "number") return isFinite(value);
    if (value === null || typeof value !== "object") return true;
    if (seen.indexOf(value) >= 0) return false;
    seen.push(value);
    var keys = Object.keys(value);
    for (var i = 0; i < keys.length; i++) if (!finiteDeep(value[keys[i]], seen)) return false;
    seen.pop();
    return true;
  }

  function rejected(code) {
    var error = new Error("寫入被拒：" + code);
    error.code = code;
    return error;
  }

  // 一筆寫入：先換本地值，再送宿主（整張表）。回傳結算的 Promise：宿主確認落檔才 resolve，被拒 reject。
  // 數值非有限、循環參照（存不成 JSON）整批拒絕，本地值不動。key＝寫入目標（事件 key 或非 message 層的 key）。
  function submit(key, rev, table, floor) {
    if (!isPlainObject(table) || !finiteDeep(table, [])) return Promise.reject(rejected("invalid-value"));
    var payload;
    try {
      payload = JSON.stringify(table);
    } catch (error) {
      return Promise.reject(rejected("invalid-value"));
    }
    var base = bases.has(key) ? bases.get(key) : rev;
    if (!oldRevs.has(key)) oldRevs.set(key, []);
    oldRevs.get(key).push(base);
    local.set(key, JSON.parse(payload));
    pending.set(key, (pending.get(key) || 0) + 1);
    sequence += 1;
    var requestId = TOKEN + ":" + String(sequence);
    var settled = new Promise(function (resolve, reject) {
      waiting.set(requestId, { key: key, resolve: resolve, reject: reject });
    });
    parentRef.postMessage(
      { source: "table-tavern-card", kind: "mvu-write", token: TOKEN, requestId: requestId, floor: floor,
        target: key, base: base, generation: store.generation, scene: store.scene, payload: payload },
      "*"
    );
    return settled;
  }
  function write(floor, table) {
    var target = targetAt(floor);
    if (!target) return Promise.reject(rejected("no-target"));
    return submit(target.key, target.rev, table, floor);
  }

  // 酒館助手變數函式（各層）。內部互叫用閉包裡的函式，卡片換掉 window 上的同名函式也不受影響。
  function getVariables(option) {
    var key = resolveLayer("getVariables", option);
    if (key !== null) {
      layerCheck("getVariables", key);
      return copy(layerData(key));
    }
    return copy(dataAt(readFloor(messageOption(option))));
  }
  function replace(variables, option) {
    var key = resolveLayer("replaceVariables", option);
    if (key !== null) {
      layerCheck("replaceVariables", key);
      return submit(key, layerRev(key), variables, -1);
    }
    return write(writeFloor(messageOption(option)), variables);
  }
  function replaceVariables(variables, option) {
    replace(variables, option).then(undefined, report);
  }
  function updateVariablesWith(updater, option) {
    var variables = getVariables(option);
    var result = updater(variables);
    if (result && typeof result.then === "function") {
      return result.then(function (value) {
        replaceVariables(value, option);
        return value;
      });
    }
    replaceVariables(result, option);
    return result;
  }
  function arraysReplace(_left, right) {
    return Array.isArray(right) ? right : undefined;
  }
  window.getVariables = getVariables;
  window.replaceVariables = replaceVariables;
  window.updateVariablesWith = updateVariablesWith;
  // 新值蓋舊值、陣列整個取代
  window.insertOrAssignVariables = function (variables, option) {
    return updateVariablesWith(function (old) {
      return lodash.mergeWith(old, variables, arraysReplace);
    }, option);
  };
  // 既有值優先：只補缺的鍵
  window.insertVariables = function (variables, option) {
    return updateVariablesWith(function (old) {
      return lodash.mergeWith({}, variables, old, arraysReplace);
    }, option);
  };
  // lodash 的 unset：路徑不存在也可能回 true，原樣交回
  window.deleteVariable = function (path, option) {
    var occurred = false;
    var variables = updateVariablesWith(function (old) {
      occurred = lodash.unset(old, path);
      return old;
    }, option);
    return { variables: variables, delete_occurred: occurred };
  };

  // MVU 的顯示字串：去掉頭尾的引號、反斜線、反引號與空白（整串含換行時照原樣）
  function trimQuotes(text) {
    if (typeof text !== "string" || text.indexOf("\\n") >= 0) return text;
    return text.replace(/^[\\\\"'\` ]+/, "").replace(/[\\\\"'\` ]+$/, "");
  }

${buildMvuParseSource()}
  var Mvu = {
    events: EVENTS,
    getMvuData: function (option) {
      return getVariables(option);
    },
    // app 加強語意：回 Promise，宿主確認落檔才 resolve、被拒 reject（上游不等落檔）
    replaceMvuData: function (data, option) {
      return replace(data, option);
    },
    // 只改傳入的 data（不送宿主）：路徑不存在回 false；舊值是長度二的陣列就當 [值, 說明] 改第 0 項；
    // 不轉型；display／delta 只更新 stat_data.$internal 那份；is_recursive 才發 SINGLE_VARIABLE_UPDATED
    setMvuVariable: async function (data, path, value, option) {
      var options = option || {};
      var stat = data && data.stat_data;
      if (!lodash.has(stat, path)) return false;
      var current = lodash.get(stat, path);
      var old;
      if (Array.isArray(current) && current.length === 2) {
        old = copy(current[0]);
        current[0] = value;
        lodash.set(stat, path, current);
      } else {
        old = copy(current);
        lodash.set(stat, path, value);
      }
      var reason = options.reason ? "(" + options.reason + ")" : "";
      var display = trimQuotes(JSON.stringify(old)) + "->" + trimQuotes(JSON.stringify(value)) + " " + reason;
      var internal = stat.$internal;
      if (internal && internal.display_data) lodash.set(internal.display_data, path, display);
      if (internal && internal.delta_data) lodash.set(internal.delta_data, path, display);
      if (options.is_recursive) await emit(EVENTS.SINGLE_VARIABLE_UPDATED, [stat, path, old, value], false);
      return true;
    },
    // 照 MVU updateVariables 重寫（card-mvu-parse-source.ts）：值解析送宿主的專用 Worker
    parseMessage: parseMessage,
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
    if (!Array.isArray(value.targets) || value.targets.length !== value.floorState.length) return false;
    if (typeof value.active !== "boolean" || typeof value.characterId !== "string") return false;
    if (!isPlainObject(value.layers)) return false;
    var layerKeys = Object.keys(value.layers);
    for (var k = 0; k < layerKeys.length; k++) {
      var layer = value.layers[layerKeys[k]];
      if (!isPlainObject(layer) || !isPlainObject(layer.vars)) return false;
      if (layer.rev !== null && typeof layer.rev !== "string") return false;
      if (layer.error !== undefined && typeof layer.error !== "string") return false;
    }
    if (!Number.isInteger(value.generation) || !Number.isInteger(value.scene)) return false;
    var macros = value.macros;
    if (!isPlainObject(macros) || typeof macros.user !== "string") return false;
    if (macros.char !== null && typeof macros.char !== "string") return false;
    for (var i = 0; i < value.states.length; i++) {
      if (!isPlainObject(value.states[i])) return false;
    }
    for (var j = 0; j < value.floorState.length; j++) {
      var index = value.floorState[j];
      if (!Number.isInteger(index) || index < 0 || index >= value.states.length) return false;
      var target = value.targets[j];
      if (target !== null && (!isPlainObject(target) || typeof target.key !== "string")) return false;
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

  function viewOf(nextStore, nextLocal, id) {
    var target = nextStore.targets[id];
    if (target && nextLocal.has(target.key)) return nextLocal.get(target.key);
    return nextStore.states[nextStore.floorState[id]];
  }

  // 換成新的快照與本地值：本樓資料沒變就只換、不發事件；變了依序等 STARTED（只帶舊資料）→ 換掉 →
  // SINGLE（逐葉）→ ENDED（新, 舊）的監聽器跑完
  async function changeTo(nextStore, nextLocal) {
    var before = viewOf(store, local, store.currentId);
    var after = viewOf(nextStore, nextLocal, nextStore.currentId);
    if (JSON.stringify(before) === JSON.stringify(after)) {
      store = nextStore;
      local = nextLocal;
      return;
    }
    await emit(EVENTS.VARIABLE_UPDATE_STARTED, [before], true);
    store = nextStore;
    local = nextLocal;
    var changes = diffLeaves(before && before.stat_data, after && after.stat_data, "", []);
    for (var i = 0; i < changes.length; i++) {
      await emit(EVENTS.SINGLE_VARIABLE_UPDATED, [after.stat_data, changes[i][0], changes[i][1], changes[i][2]], true);
    }
    await emit(EVENTS.VARIABLE_UPDATE_ENDED, [after, before], true);
  }

  // 推送：有在途寫入、或快照還是本地值之前的舊版本的目標照留本地值；其餘（確認的那版、跳版的更新版、
  // 目標已不在）丟掉本地值改讀快照
  function applyPush(next) {
    var nextLocal = new Map();
    local.forEach(function (value, key) {
      var target = isLayerKey(key)
        ? { rev: hasOwn.call(next.layers, key) ? next.layers[key].rev : null }
        : next.targets.find(function (item) {
            return item && item.key === key;
          });
      var older = target && (oldRevs.get(key) || []).indexOf(target.rev) >= 0;
      if (target && (pending.get(key) || older)) {
        nextLocal.set(key, value);
      } else {
        bases.delete(key);
        oldRevs.delete(key);
      }
    });
    return changeTo(next, nextLocal);
  }

  // 舊事件（"@位置"）第一次寫入後配到 id：這個目標的本地值、在途筆數、版本紀錄、等結算的請求與快照裡的
  // 寫入目標一起搬到新 key，之後的寫入直接寫到 id
  function migrateTarget(from, to) {
    [local, pending, bases, oldRevs].forEach(function (table) {
      if (table.has(from)) {
        table.set(to, table.get(from));
        table.delete(from);
      }
    });
    waiting.forEach(function (entry) {
      if (entry.key === from) entry.key = to;
    });
    store.targets.forEach(function (target) {
      if (target && target.key === from) target.key = to;
    });
  }

  // 結算：宿主確認或拒絕一批請求。被拒時帶回目標的權威值與版本，本地值換成它並發外部變動事件。
  function applySettle(data) {
    var authority = isPlainObject(data.authority) && typeof data.authority.key === "string" ? data.authority : null;
    var migrate = data.migrate;
    if (isPlainObject(migrate) && typeof migrate.from === "string" && typeof migrate.to === "string") {
      migrateTarget(migrate.from, migrate.to);
    }
    data.results.forEach(function (result) {
      var entry = waiting.get(result.requestId);
      if (!entry) return;
      waiting.delete(result.requestId);
      pending.set(entry.key, Math.max(0, (pending.get(entry.key) || 0) - 1));
      if (result.ok) {
        if (typeof result.rev === "string") bases.set(entry.key, result.rev);
        entry.resolve();
      } else {
        entry.reject(rejected(String(result.error || "rejected")));
      }
    });
    if (!authority) return Promise.resolve();
    pending.set(authority.key, 0);
    var nextLocal = new Map(local);
    // 沒有表（或樹模式被拒）就丟掉本地值、回到快照那一份
    if (isPlainObject(authority.table)) nextLocal.set(authority.key, copy(authority.table));
    else nextLocal.delete(authority.key);
    bases.set(authority.key, typeof authority.rev === "string" ? authority.rev : null);
    return changeTo(store, nextLocal);
  }

  var queue = Promise.resolve();
  window.addEventListener("message", function (event) {
    if (event.source !== parentRef) return;
    var data = event.data;
    if (!data || data.source !== "table-tavern-host" || data.token !== TOKEN) return;
    var work;
    // 值解析結果直接結算，不排進序列佇列：監聽器（在佇列裡跑）呼叫 parseMessage 時要等的就是它
    if (data.kind === "mvu-eval-result") {
      if (typeof data.requestId === "string") resolveEval(data);
      return;
    }
    if (data.kind === "chat") {
      if (!validMvu(data.mvu)) return;
      var next = copy(data.mvu);
      work = function () {
        return applyPush(next);
      };
    } else if (data.kind === "mvu-settle") {
      if (!Array.isArray(data.results)) return;
      var settle = copy(data);
      work = function () {
        return applySettle(settle);
      };
    } else {
      return;
    }
    queue = queue.then(work).then(undefined, report);
  });
})();
`;
}
