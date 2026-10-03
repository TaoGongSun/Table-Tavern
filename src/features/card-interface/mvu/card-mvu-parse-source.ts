// 卡片介面沙盒裡的 `Mvu.parseMessage`（計畫 8.9）：指令抽取、套用、schema 與事件分階段，照 MVU
// `updateVariables`（MagVarUpdate `src/function/update_variables.ts`、`schema.ts`，釘版本見計畫 8 節）的行為重寫，
// 只當規格書讀、不抄碼。值解析與數學式不在沙盒跑：沙盒逐條把字串送宿主的專用 Worker（card-mvu-parse-engine.ts），
// await 結果。
//
// 這份是嵌進 card-mvu-shim-source.ts 那個 IIFE 的原始碼片段（用得到它的 `emit`、`store`、`lodash`、`parentRef`、
// `TOKEN`、`EVENTS`），純 ES5＋async。用 String.raw 寫：片段裡的反斜線照原樣進沙盒，所以不能出現反引號與
// 「錢字號加左大括號」；要比對反引號寫 \x60。
export function buildMvuParseSource(): string {
  return String.raw`
  // ---- parseMessage（計畫 8.9）：上限 ----
  var PARSE_LIMITS = {
    MESSAGE_BYTES: 256 * 1024,
    COMMANDS: 1000,
    TOTAL_MS: 5000,
    TABLE_BYTES: 2 * 1024 * 1024,
    DEPTH: 32,
    STRING_BYTES: 64 * 1024,
    CHILDREN: 10000,
    NODES: 200000,
    KEY_CHARS: 256
  };
  var EXTENSIBLE_MARKER = "$__META_EXTENSIBLE__$";
  var evalWaiting = new Map();
  var evalSequence = 0;

  function parseFailure(code, message) {
    var error = new Error("parseMessage: " + message);
    error.code = code;
    return error;
  }
  function checkDeadline(ctx) {
    if (Date.now() > ctx.deadline) throw parseFailure("deadline", "整次處理超過 5 秒");
  }
  function checkCommandCount(commands) {
    if (commands.length > PARSE_LIMITS.COMMANDS) throw parseFailure("too-many-commands", "更新指令超過 1000 條");
  }
  function utf8Length(text) {
    return new TextEncoder().encode(text).length;
  }

  // 值解析請求：送宿主的專用 Worker、等結果；結果由訊息監聽器（不經序列佇列，免得監聽器裡呼叫
  // parseMessage 時卡死）交給 resolveEval。宿主 Worker 單筆 200 ms 逾時；這裡再設一道保險，宿主沒回也會結束
  function evalRemote(ctx, op, text) {
    checkDeadline(ctx);
    return new Promise(function (resolve, reject) {
      evalSequence += 1;
      var id = TOKEN + ":e" + String(evalSequence);
      var guard = setTimeout(function () {
        evalWaiting.delete(id);
        reject(parseFailure("eval-timeout", "宿主沒有回應值解析請求"));
      }, Math.max(1000, ctx.deadline - Date.now() + 500));
      evalWaiting.set(id, { resolve: resolve, guard: guard });
      parentRef.postMessage(
        { source: "table-tavern-card", kind: "mvu-eval", token: TOKEN, requestId: id, op: op, text: text },
        "*"
      );
    });
  }
  function resolveEval(data) {
    var entry = evalWaiting.get(data.requestId);
    if (!entry) return;
    evalWaiting.delete(data.requestId);
    clearTimeout(entry.guard);
    entry.resolve(data.ok === true ? { ok: true, value: data.value } : { ok: false, error: String(data.error) });
  }
  // 命令值解析：只收字串（監聽器塞進來的非字串原樣放行，同上游）；Worker 回錯（逾時、超限）算這條命令失敗
  async function parseValue(ctx, text) {
    if (typeof text !== "string") return text;
    var outcome = await evalRemote(ctx, "value", text);
    checkDeadline(ctx);
    if (!outcome.ok) {
      var failure = new Error("值解析失敗（" + outcome.error + "）");
      failure.evalFailure = true;
      throw failure;
    }
    return outcome.value;
  }
  function isoIfDate(value) {
    return Object.prototype.toString.call(value) === "[object Date]" ? value.toISOString() : value;
  }
  function isoDates(value) {
    if (Array.isArray(value)) return value.map(isoIfDate);
    return isoIfDate(value);
  }

  // 去掉頭尾的反斜線、引號、反引號與空白（含換行的字串照原樣，因為 . 不吃換行）
  function trimQuotesAndBackslashes(text) {
    if (typeof text !== "string") return text;
    return text.replace(/^[\\"'\x60 ]*(.*?)[\\"'\x60 ]*$/, "$1");
  }
  function displayOf(value) {
    return trimQuotesAndBackslashes(JSON.stringify(value));
  }
  function isVwd(value) {
    return Array.isArray(value) && value.length === 2 && typeof value[1] === "string";
  }
  function isNullOrWhiteSpace(text) {
    return text == null || String(text).trim().length === 0;
  }

  // ---- schema ----
  function isArraySchema(node) {
    return !!node && node.type === "array";
  }
  function isObjectSchema(node) {
    return !!node && node.type === "object";
  }
  function setOwn(target, key, value) {
    Object.defineProperty(target, key, { value: value, enumerable: true, writable: true, configurable: true });
  }

  function generateSchema(data, oldNode, parentRecursive) {
    parentRecursive = parentRecursive === true;
    if (oldNode === "没有用别管这个") return { type: "any" };
    if (Array.isArray(data)) {
      var extensible = false;
      var recursive = parentRecursive;
      var oldElement;
      var template;
      if (oldNode) {
        if (isArraySchema(oldNode)) {
          extensible = oldNode.extensible === true;
          recursive = oldNode.recursiveExtensible === true || parentRecursive;
          oldElement = oldNode.elementType;
          template = oldNode.template;
        } else {
          console.error("Type mismatch: expected array schema but got " + oldNode.type);
        }
      }
      // 只含 $meta 的元素（$arrayMeta）：提取陣列的元資料後從資料移除
      var metaIndex = data.findIndex(function (item) {
        return lodash.isObject(item) && !lodash.isDate(item) && "$arrayMeta" in item && "$meta" in item && item["$arrayMeta"] === true;
      });
      if (metaIndex !== -1) {
        var metaElement = data[metaIndex];
        if (metaElement.$meta.extensible !== undefined) extensible = metaElement.$meta.extensible;
        if (metaElement.$meta.template !== undefined) template = metaElement.$meta.template;
        data.splice(metaIndex, 1);
      }
      var markerIndex = data.indexOf(EXTENSIBLE_MARKER);
      if (markerIndex > -1) {
        extensible = true;
        data.splice(markerIndex, 1);
      }
      var arrayNode = {
        type: "array",
        extensible: extensible || parentRecursive,
        recursiveExtensible: recursive,
        elementType: data.length > 0 ? generateSchema(data[0], oldElement, recursive) : { type: "any" }
      };
      if (template !== undefined) arrayNode.template = template;
      return arrayNode;
    }
    if (lodash.isObject(data) && !lodash.isDate(data)) {
      var oldExtensible = false;
      var oldRecursive = parentRecursive;
      var oldProperties;
      if (oldNode) {
        if (isObjectSchema(oldNode)) {
          oldExtensible = oldNode.extensible === true;
          oldRecursive = oldNode.recursiveExtensible === true || parentRecursive;
          oldProperties = oldNode.properties;
        } else {
          console.error("Type mismatch: expected object schema but got " + oldNode.type);
        }
      }
      var meta = data.$meta;
      var node = {
        type: "object",
        properties: {},
        extensible: oldExtensible || (meta && meta.extensible === true) || (meta && meta.recursiveExtensible === true) || parentRecursive,
        recursiveExtensible: oldRecursive || (meta && meta.recursiveExtensible === true)
      };
      node.extensible = node.extensible === true;
      node.recursiveExtensible = node.recursiveExtensible === true;
      if (meta && meta.template !== undefined) node.template = meta.template;
      else if (oldNode && isObjectSchema(oldNode) && oldNode.template) node.template = oldNode.template;
      if (data.$meta) delete data.$meta;
      for (var key in data) {
        var oldChild = oldProperties ? oldProperties[key] : undefined;
        var childRecursive = node.extensible !== false && node.recursiveExtensible;
        var child = generateSchema(data[key], oldChild, childRecursive);
        // 屬性是否必填：父節點可擴充預設選填，否則必填；$meta.required 列到的強制必填；舊 schema 明說的最優先
        var required = !node.extensible;
        if (meta && Array.isArray(meta.required) && meta.required.indexOf(key) >= 0) required = true;
        if (oldChild && oldChild.required === false) required = false;
        else if (oldChild && oldChild.required === true) required = true;
        setOwn(node.properties, key, lodash.assign({}, child, { required: required }));
      }
      return node;
    }
    var kind = typeof data;
    if (kind === "string" || kind === "number" || kind === "boolean") return { type: kind };
    return { type: "any" };
  }

  // 路徑對到 schema 節點：數字段走 elementType，其餘走 properties；找不到回 null
  function getSchemaForPath(schema, path) {
    if (!path || !schema) return schema || null;
    var segments = lodash.toPath(path);
    var current = schema;
    for (var i = 0; i < segments.length; i++) {
      if (!current) return null;
      var segment = segments[i];
      if (/^\d+$/.test(segment)) {
        if (isArraySchema(current)) current = current.elementType;
        else return null;
      } else if (isObjectSchema(current) && current.properties && hasOwn.call(current.properties, segment)) {
        current = current.properties[segment];
      } else {
        return null;
      }
    }
    return current;
  }

  function reconcileAndApplySchema(variables) {
    var clone = lodash.cloneDeep(variables.stat_data);
    var next = generateSchema(clone, variables.schema);
    if (!isObjectSchema(next)) return;
    var old = variables.schema;
    if (old && old.strictTemplate !== undefined) next.strictTemplate = old.strictTemplate;
    if (old && old.strictSet !== undefined) next.strictSet = old.strictSet;
    if (old && old.concatTemplateArray !== undefined) next.concatTemplateArray = old.concatTemplateArray;
    if (lodash.has(variables.stat_data, "$meta.strictTemplate")) next.strictTemplate = variables.stat_data["$meta"].strictTemplate;
    if (lodash.has(variables.stat_data, "$meta.strictSet")) next.strictSet = variables.stat_data["$meta"].strictSet;
    if (lodash.has(variables.stat_data, "$meta.concatTemplateArray")) {
      next.concatTemplateArray = variables.stat_data["$meta"].concatTemplateArray;
    }
    variables.schema = next;
  }

  // 清掉資料裡的元資料標記：陣列的擴充標記與 $arrayMeta 元素、物件的 $meta
  function cleanUpMetadata(data) {
    if (Array.isArray(data)) {
      var i = data.length;
      while (i--) {
        var item = data[i];
        if (item === EXTENSIBLE_MARKER) {
          data.splice(i, 1);
        } else if (lodash.isObject(item) && !lodash.isDate(item) && "$arrayMeta" in item && "$meta" in item && item["$arrayMeta"] === true) {
          data.splice(i, 1);
        } else {
          cleanUpMetadata(item);
        }
      }
    } else if (lodash.isObject(data) && !lodash.isDate(data)) {
      delete data.$meta;
      for (var key in data) cleanUpMetadata(data[key]);
    }
  }

  // 套模板：值的屬性優先；型別對不上就略過模板
  function applyTemplate(value, template, strictCast, concat) {
    if (!template) return value;
    var valueIsObject = lodash.isObject(value) && !Array.isArray(value) && !lodash.isDate(value);
    var valueIsArray = Array.isArray(value);
    var templateIsArray = Array.isArray(template);
    if (valueIsObject && !templateIsArray) return lodash.merge({}, template, value);
    if (valueIsArray && templateIsArray) return concat ? lodash.concat(value, template) : lodash.merge([], template, value);
    if (
      ((valueIsObject || valueIsArray) && templateIsArray !== valueIsArray) ||
      (!valueIsObject && !valueIsArray && lodash.isObject(template) && !Array.isArray(template))
    ) {
      console.error("Template type mismatch, skipping template merge.");
      return value;
    }
    if (!valueIsObject && !valueIsArray && templateIsArray) {
      if (strictCast) return value;
      return concat ? lodash.concat([value], template) : lodash.merge([], template, [value]);
    }
    return value;
  }

  // ---- 路徑 ----
  function segmentsToPath(segments) {
    return segments
      .map(function (segment) {
        return '["' + segment.replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"]';
      })
      .join("");
  }
  function pointerToPath(pointer) {
    if (!pointer) return "";
    var body = pointer.charAt(0) === "/" ? pointer.substring(1) : pointer;
    return segmentsToPath(
      body.split("/").map(function (segment) {
        return segment.replace(/~1/g, "/").replace(/~0/g, "~");
      })
    );
  }
  // 模型常寫的路徑修正：裸數字是索引、帶引號的數字是字串鍵、含空白的鍵改成括號寫法、整段被引號包住的點分欄位去引號
  function pathFix(path) {
    if (!path) return path;
    var brackets = path.replace(/\[([^\]]*)\]/g, function (_match, rawInner) {
      var inner = rawInner.trim();
      if (!inner) return "[]";
      var wasQuoted = false;
      var first = inner.charAt(0);
      var last = inner.charAt(inner.length - 1);
      if (inner.length >= 2 && (first === '"' || first === "'") && first === last) {
        wasQuoted = true;
        inner = inner.slice(1, -1);
      }
      if (/^\d+$/.test(inner)) {
        if (!wasQuoted) return "[" + inner + "]";
        return '["' + inner.replace(/"/g, '\\"') + '"]';
      }
      if (/\s/.test(inner)) return '["' + inner.replace(/"/g, '\\"') + '"]';
      return "[" + inner + "]";
    });
    return brackets.replace(/(^|\.)(["'])([^"']*)\2(?=\.|\[|$)/g, function (_match, prefix, _quote, name) {
      if (!/\s/.test(name) && !/[.[\]]/.test(name)) return prefix + name;
      var escaped = name.replace(/"/g, '\\"');
      return prefix === "." ? '["' + escaped + '"]' : prefix + '["' + escaped + '"]';
    });
  }

  // ---- 指令抽取 ----
  function isJsonPatch(patch) {
    if (!Array.isArray(patch)) return false;
    return patch.every(function (op) {
      return (
        lodash.isPlainObject(op) &&
        typeof op.op === "string" &&
        (typeof op.path === "string" || (op.op === "move" && typeof op.to === "string"))
      );
    });
  }
  function patchToCommands(patch) {
    var out = [];
    patch.forEach(function (op) {
      var path = pointerToPath(op.path !== undefined && op.path !== null ? op.path : op.to);
      var full = JSON.stringify(op);
      switch (op.op) {
        case "replace":
          out.push({ type: "set", full_match: full, args: [path, JSON.stringify(op.value)], reason: "json_patch" });
          break;
        case "delta":
          out.push({ type: "add", full_match: full, args: [path, JSON.stringify(op.value)], reason: "json_patch" });
          break;
        case "insert":
        case "add":
          var parts = lodash.toPath(path);
          var lastPart = parts[parts.length - 1];
          var keyArg = /^\d+$/.test(lastPart) ? lastPart : JSON.stringify(lastPart);
          out.push({
            type: "insert",
            full_match: full,
            args: [segmentsToPath(parts.slice(0, -1)), keyArg, JSON.stringify(op.value)],
            reason: "json_patch"
          });
          break;
        case "remove":
          out.push({ type: "delete", full_match: full, args: [path], reason: "json_patch" });
          break;
        case "move":
          out.push({ type: "move", full_match: full, args: [pointerToPath(op.from), path], reason: "json_patch" });
          break;
      }
    });
    return out;
  }

  // 括號配對（忽略引號內的括號）：回對應右括號位置，找不到回 -1
  function findMatchingCloseParen(text, start) {
    var depth = 1;
    var inQuote = false;
    var quote = "";
    for (var i = start; i < text.length; i++) {
      var ch = text.charAt(i);
      var prev = i > 0 ? text.charAt(i - 1) : "";
      if ((ch === '"' || ch === "'" || ch === "\x60") && prev !== "\\") {
        if (!inQuote) {
          inQuote = true;
          quote = ch;
        } else if (ch === quote) {
          inQuote = false;
        }
      }
      if (!inQuote) {
        if (ch === "(") depth++;
        else if (ch === ")") {
          depth--;
          if (depth === 0) return i;
        }
      }
    }
    return -1;
  }
  // 參數切分：引號與三種括號都配對閉合時的逗號才是分隔
  function parseParameters(text) {
    var params = [];
    var current = "";
    var inQuote = false;
    var quote = "";
    var brackets = 0;
    var braces = 0;
    var parens = 0;
    for (var i = 0; i < text.length; i++) {
      var ch = text.charAt(i);
      if ((ch === '"' || ch === "'" || ch === "\x60") && (i === 0 || text.charAt(i - 1) !== "\\")) {
        if (!inQuote) {
          inQuote = true;
          quote = ch;
        } else if (ch === quote) {
          inQuote = false;
        }
      }
      if (!inQuote) {
        if (ch === "(") parens++;
        if (ch === ")") parens--;
        if (ch === "[") brackets++;
        if (ch === "]") brackets--;
        if (ch === "{") braces++;
        if (ch === "}") braces--;
      }
      if (ch === "," && !inQuote && parens === 0 && brackets === 0 && braces === 0) {
        params.push(current.trim());
        current = "";
        continue;
      }
      current += ch;
    }
    if (current.trim()) params.push(current.trim());
    return params;
  }

  function validArity(type, count) {
    if (type === "set" || type === "assign" || type === "insert") return count >= 2;
    if (type === "remove" || type === "unset" || type === "delete") return count >= 1;
    if (type === "add") return count === 2;
    return false;
  }

  async function extractCommands(ctx, text) {
    var results = [];
    // JSONPatch 區塊：內容交 Worker 解析（YAML／JSON5／修殘缺 JSON），解析失敗的區塊略過
    var patchPattern = /<(json_?patch)>(?:\s*\x60\x60\x60.*)?((?:(?!<json_?patch>)[\s\S])*?)(?:\x60\x60\x60\s*)?<\/\1>/gim;
    var blocks = [];
    var found;
    while ((found = patchPattern.exec(text)) !== null) blocks.push({ index: found.index, body: found[2].trim() });
    for (var b = 0; b < blocks.length; b++) {
      var outcome = await evalRemote(ctx, "patch", blocks[b].body);
      if (outcome.ok && isJsonPatch(outcome.value)) {
        patchToCommands(outcome.value).forEach(function (command) {
          command.$index = blocks[b].index;
          results.push(command);
        });
      }
    }
    var scan = /_\.(set|insert|assign|remove|unset|delete|add)\(/g;
    var comment = /\s*\/\/(.*)/y;
    var i = 0;
    while (i < text.length) {
      scan.lastIndex = i;
      var match = scan.exec(text);
      if (match === null) break;
      var type = match[1];
      var start = match.index;
      var open = start + match[0].length;
      var close = findMatchingCloseParen(text, open);
      if (close === -1) {
        i = open;
        continue;
      }
      var end = close + 1;
      if (end >= text.length || text.charAt(end) !== ";") {
        i = close + 1;
        continue;
      }
      end++;
      var reason = "";
      comment.lastIndex = end;
      var note = comment.exec(text);
      if (note !== null) {
        reason = note[1].trim();
        end += note[0].length;
      }
      var params = parseParameters(text.substring(open, close));
      if (validArity(type, params.length)) {
        results.push({ $index: start, type: type, full_match: text.substring(start, end), args: params, reason: reason });
        if (results.length > PARSE_LIMITS.COMMANDS) throw parseFailure("too-many-commands", "更新指令超過 1000 條");
      }
      i = end;
    }
    checkCommandCount(results);
    return lodash.sortBy(results, "$index").map(function (command) {
      return lodash.omit(command, "$index");
    });
  }

  function substituteMacros(text) {
    var macros = store.macros;
    if (!macros) return text;
    return text.replace(/\{\{(user|char)\}\}/gi, function (match, name) {
      if (name.toLowerCase() === "user") return macros.user;
      return macros.char === null || macros.char === undefined ? match : macros.char;
    });
  }

  // ---- 套用指令 ----
  function outError(st, content) {
    var command = st.command ? st.command.full_match : "（未知指令）";
    console.warn("變數更新出錯：" + command + "\n" + content);
    st.error = { command: command, content: content };
  }
  async function singleUpdated(st, path, oldValue, newValue) {
    await emit(EVENTS.SINGLE_VARIABLE_UPDATED, [st.variables.stat_data, path, oldValue, newValue], false);
    checkDeadline(st.ctx);
  }

  async function applySet(st, command, path, reasonStr) {
    var variables = st.variables;
    if (path !== "" && !lodash.has(variables.stat_data, path)) {
      outError(st, "set 的路徑不存在：" + path + " " + reasonStr);
      return "";
    }
    var oldValue = path === "" ? lodash.cloneDeep(variables.stat_data) : lodash.get(variables.stat_data, path);
    var newValue = isoIfDate(await parseValue(st.ctx, command.args[command.args.length - 1]));
    var pathIsVwd = false;
    if (!st.strictSet && isVwd(oldValue) && !Array.isArray(oldValue[0])) {
      // [值, 說明]：改第 0 項；舊值是數字且新值不是 null 才強轉成數字（數字欄位可以設成 null）
      var oldCopy = lodash.cloneDeep(oldValue[0]);
      oldValue[0] = typeof oldValue[0] === "number" && newValue !== null ? Number(newValue) : newValue;
      oldValue = oldCopy;
      pathIsVwd = true;
    } else if (typeof oldValue === "number" && newValue !== null && typeof newValue === "string") {
      lodash.set(variables.stat_data, path, Number(newValue));
    } else if (path) {
      lodash.set(variables.stat_data, path, newValue);
    } else {
      variables.stat_data = newValue;
    }
    var finalValue = path === "" ? variables.stat_data : lodash.get(variables.stat_data, path);
    if (pathIsVwd) finalValue = finalValue[0];
    var display;
    if (!st.strictSet && isVwd(oldValue) && Array.isArray(finalValue)) {
      display = displayOf(oldValue[0]) + "->" + displayOf(finalValue[0]) + " " + reasonStr;
    } else {
      display = displayOf(oldValue) + "->" + displayOf(finalValue) + " " + reasonStr;
    }
    await singleUpdated(st, path, oldValue, finalValue);
    return display;
  }

  async function applyInsert(st, command, path, reasonStr) {
    var variables = st.variables;
    var schema = variables.schema;
    var existing = path === "" ? variables.stat_data : lodash.get(variables.stat_data, path);
    var targetSchema = getSchemaForPath(schema, path);
    // 目標是原始型別就不能插入
    if (existing !== null && !Array.isArray(existing) && !lodash.isObject(existing)) {
      outError(st, "insert 的目標是原始型別（" + typeof existing + "）：" + path + " " + reasonStr);
      return "";
    }
    if (targetSchema) {
      if (targetSchema.type === "object" && targetSchema.extensible === false) {
        if (command.args.length === 2) {
          outError(st, "不可擴充的物件不能合併：" + path + " " + reasonStr);
          return "";
        }
        if (command.args.length >= 3) {
          var newKey = String(await parseValue(st.ctx, command.args[1]));
          if (!lodash.has(targetSchema.properties, newKey)) {
            outError(st, "不可擴充的物件沒有這個鍵：" + newKey + "（" + path + "） " + reasonStr);
            return "";
          }
        }
      } else if (targetSchema.type === "array" && (targetSchema.extensible === false || targetSchema.extensible === undefined)) {
        outError(st, "不可擴充的陣列不能插入：" + path + " " + reasonStr);
        return "";
      }
    } else if (path !== "" && !lodash.get(variables.stat_data, segmentsToPath(lodash.toPath(path).slice(0, -1)))) {
      outError(st, "插入的父路徑不存在：" + path + " " + reasonStr);
      return "";
    }
    var oldValue = lodash.cloneDeep(existing);
    var display = "";
    var successful = false;
    var collection;
    var template;
    var valueToAssign;
    if (command.args.length === 2) {
      valueToAssign = isoDates(await parseValue(st.ctx, command.args[1]));
      collection = path === "" ? variables.stat_data : lodash.get(variables.stat_data, path);
      if (!Array.isArray(collection) && !lodash.isObject(collection)) {
        collection = Array.isArray(valueToAssign) ? [] : {};
        lodash.set(variables.stat_data, path, collection);
      }
      if (Array.isArray(collection)) {
        template = targetSchema && isArraySchema(targetSchema) ? targetSchema.template : undefined;
        valueToAssign = applyTemplate(valueToAssign, template, st.strictTemplate, st.concatTemplateArray);
        collection.push(valueToAssign);
        display = "ASSIGNED " + JSON.stringify(valueToAssign) + " into array '" + path + "' " + reasonStr;
        successful = true;
      } else if (lodash.isObject(collection)) {
        if (lodash.isObject(valueToAssign) && !Array.isArray(valueToAssign)) {
          lodash.merge(collection, valueToAssign);
          display = "MERGED object " + JSON.stringify(valueToAssign) + " into object '" + path + "' " + reasonStr;
          successful = true;
        } else {
          outError(st, (Array.isArray(valueToAssign) ? "不能把陣列合併進物件：" : "不能把非物件合併進物件：") + path);
          return "";
        }
      }
    } else if (command.args.length >= 3) {
      valueToAssign = isoDates(await parseValue(st.ctx, command.args[2]));
      var keyOrIndex = await parseValue(st.ctx, command.args[1]);
      collection = path === "" ? variables.stat_data : lodash.get(variables.stat_data, path);
      template = targetSchema && (isArraySchema(targetSchema) || isObjectSchema(targetSchema)) ? targetSchema.template : undefined;
      if (Array.isArray(collection) && (typeof keyOrIndex === "number" || keyOrIndex === "-")) {
        // JSON Patch 的 "-" 是追加到陣列尾
        var index = keyOrIndex === "-" ? collection.length : keyOrIndex;
        var label = keyOrIndex === "-" || keyOrIndex === -1 ? "tail" : keyOrIndex;
        valueToAssign = applyTemplate(valueToAssign, template, st.strictTemplate, st.concatTemplateArray);
        collection.splice(index, 0, valueToAssign);
        display = "ASSIGNED " + JSON.stringify(valueToAssign) + " into '" + path + "' at index " + label + " " + reasonStr;
        successful = true;
      } else if (lodash.isObject(collection)) {
        valueToAssign = applyTemplate(valueToAssign, template, st.strictTemplate, st.concatTemplateArray);
        setOwn(collection, String(keyOrIndex), valueToAssign);
        display = "ASSIGNED key '" + keyOrIndex + "' with value " + JSON.stringify(valueToAssign) + " into object '" + path + "' " + reasonStr;
        successful = true;
      } else {
        collection = {};
        lodash.set(variables.stat_data, path, collection);
        valueToAssign = applyTemplate(valueToAssign, template, st.strictTemplate, st.concatTemplateArray);
        setOwn(collection, String(keyOrIndex), valueToAssign);
        display = "CREATED object at '" + path + "' and ASSIGNED key '" + keyOrIndex + "' " + reasonStr;
        successful = true;
      }
    }
    if (!successful) {
      outError(st, "insert 的參數無效：" + path);
      return "";
    }
    var newValue = isNullOrWhiteSpace(path) ? variables.stat_data : lodash.get(variables.stat_data, path);
    await singleUpdated(st, path, oldValue, newValue);
    try {
      // 新套用的模板立刻展開成 schema，並清掉資料裡的元資料標記
      var clone = lodash.cloneDeep(newValue);
      lodash.merge(targetSchema, generateSchema(clone, targetSchema));
      cleanUpMetadata(newValue);
    } catch (error) {
      outError(st, "模板展開失敗（" + path + "）：" + (error instanceof Error ? error.message : String(error)));
    }
    return display;
  }

  async function applyDelete(st, command, path, reasonStr) {
    var variables = st.variables;
    var schema = variables.schema;
    var parts = lodash.toPath(path);
    var lastPart = parts[parts.length - 1];
    // 單一參數、路徑尾是數字：移除那個陣列元素
    if (command.args.length === 1 && /^\d+$/.test(lastPart)) {
      var arrayPath = segmentsToPath(parts.slice(0, -1));
      var container = lodash.get(variables.stat_data, arrayPath);
      var at = parseInt(lastPart, 10);
      if (Array.isArray(container) && at < container.length) {
        var originalArray = lodash.cloneDeep(container);
        container.splice(at, 1);
        await singleUpdated(st, arrayPath, originalArray, container);
        return "";
      }
    }
    if (!lodash.has(variables.stat_data, path)) {
      outError(st, "delete 的路徑不存在：" + path);
      return "";
    }
    var containerPath = path;
    var keyOrIndex;
    if (command.args.length > 1) {
      keyOrIndex = await parseValue(st.ctx, command.args[1]);
      if (typeof keyOrIndex === "string") keyOrIndex = trimQuotesAndBackslashes(keyOrIndex);
    } else {
      var segments = lodash.toPath(path);
      var tail = segments.pop();
      if (tail) {
        keyOrIndex = /^\d+$/.test(tail) ? Number(tail) : tail;
        containerPath = segmentsToPath(segments);
      }
    }
    if (keyOrIndex === undefined) {
      outError(st, "delete 無法判斷刪除目標：" + path + " " + reasonStr);
      return "";
    }
    if (containerPath !== "" && !lodash.has(variables.stat_data, containerPath)) {
      outError(st, "delete 的容器路徑不存在：" + containerPath + " " + reasonStr);
      return "";
    }
    var containerSchema = getSchemaForPath(schema, containerPath);
    if (containerSchema) {
      if (containerSchema.type === "array") {
        if (containerSchema.extensible !== true) {
          outError(st, "不可擴充的陣列不能刪除元素：" + containerPath + " " + reasonStr);
          return "";
        }
      } else if (containerSchema.type === "object") {
        var keyString = String(keyOrIndex);
        if (lodash.has(containerSchema.properties, keyString) && containerSchema.properties[keyString].required === true) {
          outError(st, "不能刪除必填的鍵：" + keyString + "（" + containerPath + "） " + reasonStr);
          return "";
        }
      }
    }
    var target = command.args.length > 1 ? await parseValue(st.ctx, command.args[1]) : undefined;
    var removed = false;
    var display = "";
    if (target === undefined) {
      var oldValue = lodash.get(variables.stat_data, path);
      lodash.unset(variables.stat_data, path);
      display = "REMOVED path '" + path + "' " + reasonStr;
      removed = true;
      await singleUpdated(st, path, oldValue, undefined);
    } else {
      var collection = lodash.get(variables.stat_data, path);
      if (!Array.isArray(collection) && !lodash.isObject(collection)) {
        outError(st, "delete 的目標不是集合：" + path + " " + reasonStr);
        return "";
      }
      if (Array.isArray(collection)) {
        var original = lodash.cloneDeep(collection);
        var indexToRemove = -1;
        if (typeof target === "number") {
          indexToRemove = target;
        } else {
          indexToRemove = collection.findIndex(function (item) {
            return lodash.isEqual(item, target);
          });
        }
        if (indexToRemove >= 0 && indexToRemove < collection.length) {
          collection.splice(indexToRemove, 1);
          removed = true;
          display = "REMOVED item from '" + path + "' " + reasonStr;
          await singleUpdated(st, path, original, collection);
        }
      } else if (lodash.isObject(collection)) {
        if (typeof target === "number") {
          var keys = Object.keys(collection);
          if (target >= 0 && target < keys.length) {
            var byIndex = keys[target];
            lodash.unset(collection, byIndex);
            removed = true;
            display = "REMOVED " + (target + 1) + "th entry ('" + byIndex + "') from object '" + path + "' " + reasonStr;
          }
        } else {
          var byName = String(target);
          if (lodash.has(collection, byName)) {
            delete collection[byName];
            removed = true;
            display = "REMOVED key '" + byName + "' from object '" + path + "' " + reasonStr;
          }
        }
      }
    }
    if (!removed) {
      outError(st, "delete 沒有刪到東西：" + path);
      return "";
    }
    return display;
  }

  async function applyAdd(st, command, path, reasonStr) {
    var variables = st.variables;
    if (!lodash.has(variables.stat_data, path)) {
      outError(st, "add 的路徑不存在：" + path + " " + reasonStr);
      return "";
    }
    var initial = lodash.cloneDeep(lodash.get(variables.stat_data, path));
    var oldValue = lodash.get(variables.stat_data, path);
    var valueToAdd = oldValue;
    var vwd = isVwd(oldValue) && typeof oldValue[0] !== "object";
    if (vwd) valueToAdd = oldValue[0];
    // 目前值（或字串）能當日期就走日期加毫秒；純數字字串不算日期
    var date = null;
    if (Object.prototype.toString.call(valueToAdd) === "[object Date]") {
      date = valueToAdd;
    } else if (typeof valueToAdd === "string") {
      var parsed = new Date(valueToAdd);
      if (!isNaN(parsed.getTime()) && isNaN(Number(valueToAdd))) date = parsed;
    }
    if (command.args.length !== 2) {
      outError(st, "add 的參數無效：" + path + " " + reasonStr);
      return "";
    }
    var delta = await parseValue(st.ctx, command.args[1]);
    var finalValue;
    var display;
    if (date) {
      if (typeof delta !== "number") {
        outError(st, "日期的增量必須是毫秒數：" + command.args[1] + " " + reasonStr);
        return "";
      }
      var shifted = new Date(date.getTime() + delta).toISOString();
      if (vwd) {
        oldValue[0] = shifted;
        lodash.set(variables.stat_data, path, oldValue);
      } else {
        lodash.set(variables.stat_data, path, shifted);
      }
    } else if (typeof valueToAdd === "number") {
      if (typeof delta !== "number") {
        outError(st, "增量必須是數字：" + command.args[1] + " " + reasonStr);
        return "";
      }
      var sum = parseFloat((valueToAdd + delta).toPrecision(12));
      if (vwd) {
        oldValue[0] = sum;
        lodash.set(variables.stat_data, path, oldValue);
      } else {
        lodash.set(variables.stat_data, path, sum);
      }
    } else {
      outError(st, "add 不支援這種值：" + path + " " + reasonStr);
      return "";
    }
    finalValue = lodash.get(variables.stat_data, path);
    display = vwd
      ? JSON.stringify(initial[0]) + "->" + JSON.stringify(finalValue[0]) + " " + reasonStr
      : JSON.stringify(initial) + "->" + JSON.stringify(finalValue) + " " + reasonStr;
    await singleUpdated(st, path, initial, finalValue);
    return display;
  }

  // MVU updateVariables：STARTED → 抽指令 → COMMAND_PARSED（含 _for_zod 兩個變體）→ 逐條套用（每條
  // SINGLE_VARIABLE_UPDATED）→ ENDED → schema 調和 → ENDED_for_zod。事件監聽器依序 await，參數是活物件
  async function updateVariables(ctx, content, variables) {
    var before = lodash.cloneDeep(variables);
    var outStatus = lodash.cloneDeep(variables);
    var deltaStatus = { stat_data: {} };
    var commands = await extractCommands(ctx, substituteMacros(content));
    lodash.set(variables.stat_data, "$internal", {
      display_data: outStatus.stat_data,
      delta_data: deltaStatus.stat_data
    });
    await emit(EVENTS.VARIABLE_UPDATE_STARTED, [variables], false);
    checkDeadline(ctx);
    var schema = variables.schema;
    var st = {
      ctx: ctx,
      variables: variables,
      command: null,
      error: null,
      strictTemplate: (schema && schema.strictTemplate) || false,
      concatTemplateArray: schema && schema.concatTemplateArray !== undefined ? schema.concatTemplateArray : true,
      strictSet: (schema && schema.strictSet) || false
    };
    commands.forEach(function (command) {
      if (command.type === "remove" || command.type === "unset") command.type = "delete";
      else if (command.type === "assign") command.type = "insert";
    });
    await emit(EVENTS.COMMAND_PARSED, [variables, commands, content], false);
    await emit(EVENTS.COMMAND_PARSED + "_for_zod", [variables, commands, content], false);
    await emit(EVENTS.COMMAND_PARSED + "_ended_for_zod", [variables, commands, content], false);
    // 監聽器可以增刪指令：套用前重查指令數與期限
    checkCommandCount(commands);
    checkDeadline(ctx);
    for (var c = 0; c < commands.length; c++) {
      var command = commands[c];
      checkDeadline(ctx);
      checkCommandCount(commands);
      // JSON Patch 的路徑已照 JSON Pointer 編好，不再修正
      if (command.reason !== "json_patch") command.args[0] = pathFix(trimQuotesAndBackslashes(command.args[0]));
      var path = command.args[0];
      var reasonStr = command.reason ? "(" + command.reason + ")" : "";
      var display = "";
      st.command = command;
      try {
        // 別名在 COMMAND_PARSED 之前就換過了，但監聽器之後加進來的指令仍可能是別名寫法
        var kind = command.type;
        if (kind === "set") display = await applySet(st, command, path, reasonStr);
        else if (kind === "insert" || kind === "assign") display = await applyInsert(st, command, path, reasonStr);
        else if (kind === "delete" || kind === "remove" || kind === "unset") display = await applyDelete(st, command, path, reasonStr);
        else if (kind === "add") display = await applyAdd(st, command, path, reasonStr);
      } catch (error) {
        if (error && error.evalFailure) {
          outError(st, error.message);
          continue;
        }
        throw error;
      }
      if (display) {
        lodash.set(outStatus.stat_data, path, display);
        lodash.set(deltaStatus.stat_data, path, display);
      }
    }
    variables.display_data = outStatus.stat_data;
    variables.delta_data = deltaStatus.stat_data;
    await emit(EVENTS.VARIABLE_UPDATE_ENDED, [variables, before], false);
    checkDeadline(ctx);
    lodash.unset(variables.stat_data, "$internal");
    var modified = !lodash.isEqual(variables.stat_data, before.stat_data);
    if (modified) reconcileAndApplySchema(variables);
    await emit(EVENTS.VARIABLE_UPDATE_ENDED + "_for_zod", [variables, before], false);
    checkDeadline(ctx);
    return modified;
  }

  // 結果的上限（計畫 8.8）：整張表序列化後大小、深度、字串、元素數、節點數、鍵長，數值必須有限；符合回 null
  function limitProblem(table) {
    var payload;
    try {
      payload = JSON.stringify(table);
    } catch (error) {
      return "invalid-value";
    }
    if (payload === undefined) return "invalid-value";
    if (utf8Length(payload) > PARSE_LIMITS.TABLE_BYTES) return "too-large";
    var nodes = 0;
    function walk(node, depth) {
      nodes += 1;
      if (nodes > PARSE_LIMITS.NODES) return "too-many-nodes";
      if (depth > PARSE_LIMITS.DEPTH) return "too-deep";
      if (typeof node === "string") return utf8Length(node) > PARSE_LIMITS.STRING_BYTES ? "string-too-long" : null;
      if (typeof node === "number") return isFinite(node) ? null : "non-finite";
      if (node === null || typeof node !== "object") return null;
      var keys = Object.keys(node);
      // 陣列看 length（空洞不算在 keys 裡）
      if (Array.isArray(node) && node.length > PARSE_LIMITS.CHILDREN) return "too-many-children";
      if (keys.length > PARSE_LIMITS.CHILDREN) return "too-many-children";
      for (var i = 0; i < keys.length; i++) {
        if (!Array.isArray(node)) {
          if (keys[i] === "") return "empty-key";
          if (Array.from(keys[i]).length > PARSE_LIMITS.KEY_CHARS) return "key-too-long";
        }
        var problem = walk(node[keys[i]], depth + 1);
        if (problem) return problem;
      }
      return null;
    }
    return walk(table, 1);
  }

  // Mvu.parseMessage(message, old)：深拷貝 old 後照訊息更新、回新資料（上游一律回新資料，沒有變動也是）。
  // 純沙盒運算，不送宿主寫入、不落檔、不記帳；超過上限（訊息 256 KB、指令 1000 條、整次 5 秒、結果同 8.8）丟錯
  async function parseMessage(message, oldData) {
    if (typeof message !== "string") throw new Error("parseMessage: message 必須是字串");
    if (!isPlainObject(oldData)) throw new Error("parseMessage: old_data 必須是物件");
    if (utf8Length(message) > PARSE_LIMITS.MESSAGE_BYTES) throw parseFailure("message-too-large", "訊息超過 256 KB");
    var ctx = { deadline: Date.now() + PARSE_LIMITS.TOTAL_MS };
    var result = lodash.cloneDeep(oldData);
    await updateVariables(ctx, message, result);
    checkDeadline(ctx);
    var problem = limitProblem(result);
    if (problem) throw parseFailure(problem, "結果超出上限（" + problem + "）");
    return result;
  }
`;
}
