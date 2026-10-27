// MVU 共用核心片段：zod 版 MVU 卡（mvu_zod）的 schema 註冊、開局驗證與逐條套用。片段規則與串接順序見 engine-source.ts。
// 行為照 StageDog/tavern_resource `util/mvu_zod.ts`（29becf3），只當規格書讀、不抄碼。唯一刻意偏離：指令值裡的
// 物件／陣列字面量上游用 new Function 求值，這裡改 JSON5（模型輸出永不當程式執行），見 value-parser.ts。
export const SOURCE_ZOD = String.raw`
  // ---- zod（mvu_zod）----
  var ZOD_PLACEHOLDER_SCHEMA = "没有用别管这个";
  var ZOD_LIMITS = { ISSUES: 5, TEXT_CHARS: 500, CHANGES: 2000, CHANGE_VALUE_BYTES: 8192 };
  var zodState = { input: null, duplicates: 0 };

  // 卡片 schema 腳本 import 的 registerMvuSchema：只認第一份，之後的計數（宿主據以明示拒絕）。照上游註冊時就展開
  // 一次（傳函式的卡此時就會執行它），展開拋錯就不算註冊、照拋（宿主當載入失敗）
  function registerMvuSchema(input) {
    if (zodState.input !== null) {
      zodState.duplicates += 1;
      return;
    }
    zodState.input = input;
    try {
      zodSchema();
    } catch (error) {
      zodState.input = null;
      throw error;
    }
  }
  function zodStatus() {
    return { registered: zodState.input !== null, duplicates: zodState.duplicates };
  }
  function zodActive() {
    return zodState.input !== null && !!env.z;
  }
  // 每次用到都重新展開（傳函式的卡每次重建）；頂層 z.object 換成 looseObject，保留 schema 沒列的頂層鍵
  function zodSchema() {
    var z = env.z;
    var original = typeof zodState.input === "function" ? zodState.input() : zodState.input;
    return z && original instanceof z.ZodObject ? z.looseObject(original.shape) : original;
  }

  // 超過字數就切到上限內、尾端加「…」當截斷標記（加上標記後仍不超過上限）
  function clipText(text) {
    var value = String(text);
    return value.length > ZOD_LIMITS.TEXT_CHARS ? value.slice(0, ZOD_LIMITS.TEXT_CHARS - 1) + "…" : value;
  }
  // 錯誤文字裡的路徑：鍵名含點、括號或引號的段用 ["…"] 包起來，免得對不到欄位
  function formatPath(segments) {
    return segments
      .map(function (segment, i) {
        var key = String(segment);
        if (/^[^.[\]"'\\]+$/.test(key)) return i === 0 ? key : "." + key;
        return "[" + JSON.stringify(key) + "]";
      })
      .join("");
  }
  function jsonOrString(value) {
    try {
      var text = JSON.stringify(value);
      return text === undefined ? String(value) : text;
    } catch (error) {
      return String(value);
    }
  }
  // 驗證錯誤：照上游 prettifyErrorWithInput 的內容（路徑短的在前；訊息、路徑、輸入），每則一條、限條數與字數
  function zodIssues(error) {
    var issues = lodash.sortBy(error.issues.slice(), function (issue) {
      return issue.path ? issue.path.length : 0;
    });
    var lines = issues.slice(0, ZOD_LIMITS.ISSUES).map(function (issue) {
      var head = (issue.path && issue.path.length ? "路径: " + formatPath(issue.path) + " " : "") + "✖ " + issue.message;
      return issueLine(head, issue.input, "输入");
    });
    return { issues: lines, issueCount: issues.length };
  }
  // 一則錯誤：路徑與訊息在前、輸入在後，超過字數只截輸入那段（路徑永遠看得到）
  function issueLine(head, input, name) {
    if (input === undefined) return clipText(head);
    var label = " → " + (name || "输入") + ": ";
    var room = ZOD_LIMITS.TEXT_CHARS - head.length - label.length;
    if (room < 2) return clipText(head);
    var text = jsonOrString(input);
    return head + label + (text.length > room ? text.slice(0, room - 1) + "…" : text);
  }
  function exceptionText(error) {
    if (error && error.stack) return clipText(error.stack);
    return clipText(error && error.name ? error.name + ": " + error.message : String(error));
  }

  // 宿主判定的資源類錯誤（記憶體、堆疊觸限）照拋、整次失敗，不當單條拒收
  function rethrowFatal(error) {
    if (host.fatal && host.fatal(error)) throw error;
  }
  // 整份 stat_data 過 schema：成功回 { data }，失敗回 { issues, issueCount }（transform 拋錯也算失敗）
  function zodCheck(schema, data) {
    try {
      var result = schema.safeParse(data, { reportInput: true });
      if (result.success) return { data: result.data };
      return zodIssues(result.error);
    } catch (error) {
      rethrowFatal(error);
      return { issues: [exceptionText(error)], issueCount: 1 };
    }
  }

  function zodPath(text) {
    return trimQuotesAndBackslashes(String(text)).replace(/^(?:stat_data|status_current_variables)\./, "");
  }
  function zodReadonly(path) {
    var segments = typeof path === "string" ? lodash.toPath(path) : path;
    return segments.some(function (segment) {
      return String(segment).charAt(0) === "_";
    });
  }
  // 指令值：mvu_zod 自己的解析（沒有數學式），字串才送宿主；監聽器塞進來的非字串原樣放行。zod 版的解析本身
  // 不會失敗，宿主回錯一定是資源問題（逾時、值超過上限、Worker 掛掉）：整次失敗，不當單條拒收
  async function zodValue(ctx, text) {
    if (typeof text !== "string") return text;
    var outcome = await evalRemote(ctx, "zod-value", text);
    checkDeadline(ctx);
    if (!outcome.ok) throw parseFailure("eval-failed", "值解析失敗（" + outcome.error + "）");
    return outcome.value;
  }

  // 套一條：回 { data } 或 { data: null, reported, reason, issues?, issueCount? }。reported＝上游會通知 GM 的拒收；
  // 沒 reported 的是上游靜默丟掉的（唯讀路徑、目標不是容器、不認得的指令）
  function zodRejected(check) {
    return { data: null, reported: true, reason: "schema", issues: check.issues, issueCount: check.issueCount };
  }
  function zodIgnored(reason) {
    return { data: null, reported: false, reason: reason };
  }
  async function zodApply(ctx, schema, data, command) {
    var args = command.args.slice();
    var path;
    var check;
    if (command.type === "set") {
      if (args.length === 3) args.splice(1, 1);
      path = zodPath(args[0]);
      if (zodReadonly(path)) return zodIgnored("readonly");
      var newValue = await zodValue(ctx, args[1]);
      if (path) ownSet(data, path, newValue);
      else data = newValue;
      check = zodCheck(schema, data);
      return check.data !== undefined ? { data: check.data } : zodRejected(check);
    }
    if (command.type === "add") {
      path = zodPath(args[0]);
      if (!path) return zodIgnored("no-path");
      if (zodReadonly(path)) return zodIgnored("readonly");
      var oldValue = ownGet(data, path);
      if (typeof oldValue !== "number") {
        return {
          data: null,
          reported: true,
          reason: "add-not-number",
          issues: [issueLine("路径: " + formatPath(pathSegments(path)) + " ✖ 不能对非数字类型 '" + typeof oldValue + "' 进行加减运算", oldValue, "原值")],
          issueCount: 1
        };
      }
      var delta = await zodValue(ctx, args[1]);
      ownSet(data, path, oldValue + Number(delta));
      check = zodCheck(schema, data);
      return check.data !== undefined ? { data: check.data } : zodRejected(check);
    }
    if (command.type === "insert") {
      path = zodPath(args[0]);
      if (zodReadonly(path)) return zodIgnored("readonly");
      var keyOrIndex = await zodValue(ctx, args[1]);
      var value = args.length === 2 ? keyOrIndex : await zodValue(ctx, args[args.length - 1]);
      var insertInto = function (target) {
        var collection = path === "" ? target : ownGet(target, path);
        if (args.length === 2) {
          if (Array.isArray(collection)) collection.push(value);
          else lodash.assign(collection, value);
        } else if (Array.isArray(collection)) {
          collection.splice(keyOrIndex === "-" ? collection.length : keyOrIndex, 0, value);
        } else {
          ownPut(collection, keyOrIndex, value);
        }
        return zodCheck(schema, target);
      };
      var existing = path === "" ? data : ownGet(data, path);
      var missing = lodash.isNil(existing);
      if (!missing && !Array.isArray(existing) && !lodash.isPlainObject(existing)) return zodIgnored("not-container");
      if (!missing) {
        check = insertInto(data);
        return check.data !== undefined ? { data: check.data } : zodRejected(check);
      }
      // 容器不存在：先當物件建，驗不過再當陣列建（同一份拷貝），只回報最後那次的失敗
      var filled = lodash.cloneDeep(data);
      ownSet(filled, path, {});
      check = insertInto(filled);
      if (check.data !== undefined) return { data: check.data };
      ownSet(filled, path, []);
      check = insertInto(filled);
      return check.data !== undefined ? { data: check.data } : zodRejected(check);
    }
    if (command.type === "delete") {
      var segments = lodash.toPath(args.map(zodPath).join("."));
      if (zodReadonly(segments)) return zodIgnored("readonly");
      var parent = ownGet(data, lodash.dropRight(segments));
      if (Array.isArray(parent)) lodash.pullAt(parent, Number(lodash.last(segments)));
      else ownUnset(data, segments);
      check = zodCheck(schema, data);
      return check.data !== undefined ? { data: check.data } : zodRejected(check);
    }
    return zodIgnored("unsupported");
  }

  // 指令作用的路徑（分段陣列，保留完整鍵名）：delete 照上游把所有參數接成一條路徑；move 另記目的地
  // 指令作用的路徑，分段要和實際寫入時一樣：set／add／insert 照 pathSegments 對這條指令開始時的表解析（整串是
  // 自有鍵就當單一鍵）；delete 與 move 的來源照上游一律 toPath 拆段
  function commandPath(command, root) {
    if (!command.args.length) return [];
    if (command.type === "delete") return lodash.toPath(command.args.map(zodPath).join("."));
    if (command.type === "move") return lodash.toPath(zodPath(command.args[0]));
    return pathSegments(zodPath(command.args[0]), root);
  }
  function zodResult(index, command, status, outcome, root) {
    var entry = { index: index, op: command.type, path: commandPath(command, root), status: status };
    if (command.type === "move" && command.args.length > 1) entry.to = pathSegments(zodPath(command.args[1]), root);
    if (outcome && outcome.reason) entry.reason = outcome.reason;
    if (outcome && outcome.issues) {
      entry.issues = outcome.issues;
      entry.issueCount = outcome.issueCount;
    }
    return entry;
  }

  // COMMAND_PARSED_for_zod：每條各自在 stat_data 的深拷貝上套用、整份過 schema，通過就把結果淺合併回 stat_data
  async function zodCommandParsed(ctx, variables, commands) {
    var schema = zodSchema();
    var consumed = [];
    for (var index = 0; index < commands.length; index++) {
      checkDeadline(ctx);
      var command = commands[index];
      var root = variables.stat_data;
      var data = lodash.cloneDeep(root);
      var outcome;
      if (command.type === "move") {
        var fromPath = zodPath(command.args[0]);
        if (!ownHas(data, fromPath)) {
          outcome = {
            data: null,
            reported: true,
            reason: "move-source-missing",
            issues: [clipText("移动源路径不存在: " + formatPath(pathSegments(fromPath)))],
            issueCount: 1
          };
        } else {
          var moved = ownGet(data, fromPath);
          outcome = await zodApply(ctx, schema, data, { type: "delete", args: [fromPath] });
          // 刪除來源就失敗：上游仍對 null 做 set、最後報 schema 錯；資料一樣不寫，但原因記刪除那一步的
          if (outcome.data === null) {
            outcome = {
              data: null,
              reported: true,
              reason: "move-source-" + outcome.reason,
              issues: outcome.issues,
              issueCount: outcome.issueCount
            };
          } else {
            outcome = await zodApply(ctx, schema, outcome.data, { type: "set", args: [zodPath(command.args[1]), moved] });
          }
        }
      } else {
        outcome = await zodApply(ctx, schema, data, command);
      }
      checkDeadline(ctx);
      var next = outcome.data;
      if (next === null || next === undefined) {
        ctx.zodResults.push(zodResult(index, command, outcome.reported ? "rejected" : "ignored", outcome, root));
        continue;
      }
      // 頂層 delete／move 會讓結果少一個鍵：只比結果現有的鍵會誤判沒變，所以被移除的來源路徑另外比、合併前先清掉
      var removedPaths = command.type === "delete" || command.type === "move" ? [commandPath(command, root)] : [];
      var removed = removedPaths.some(function (removedPath) {
        return ownHas(variables.stat_data, removedPath) && !ownHas(next, removedPath);
      });
      if (removed || !lodash.isEqual(next, ownPick(variables.stat_data, Object.keys(next)))) {
        removedPaths.forEach(function (removedPath) {
          if (!ownHas(next, removedPath)) ownUnset(variables.stat_data, removedPath);
        });
        variables.stat_data = lodash.assign({}, variables.stat_data, next);
        consumed.push(index);
        ctx.zodResults.push(zodResult(index, command, "applied", null, root));
      } else {
        ctx.zodResults.push(zodResult(index, command, "no-change", { reason: "unchanged" }, root));
      }
    }
    lodash.pullAt(commands, consumed);
  }

  // VARIABLE_UPDATE_ENDED_for_zod：schema 換成佔位（之後 $meta 不生效）、不留 display／delta
  function zodUpdateEnded(variables) {
    lodash.set(variables, "schema", ZOD_PLACEHOLDER_SCHEMA);
    lodash.unset(variables, "display_data");
    lodash.unset(variables, "delta_data");
  }

  // 開局（VARIABLE_INITIALIZED）：整份 stat_data 過 schema，成功淺合併回去（補預設、轉型、transform），失敗照原值、回錯誤
  function zodInitialize(variables) {
    if (!zodActive()) return { ok: true, zod: false };
    var check;
    try {
      check = zodCheck(zodSchema(), lodash.get(variables, "stat_data", {}));
    } catch (error) {
      rethrowFatal(error);
      return { ok: false, zod: true, issues: [exceptionText(error)], issueCount: 1 };
    }
    if (check.data === undefined) return { ok: false, zod: true, issues: check.issues, issueCount: check.issueCount };
    variables.stat_data = lodash.assign({}, variables.stat_data, check.data);
    nullNonFinite(variables.stat_data);
    return { ok: true, zod: true };
  }

  // transform 留下的非有限數照上游存檔（JSON.stringify）寫成 null
  function nullNonFinite(node) {
    if (Array.isArray(node)) {
      for (var i = 0; i < node.length; i++) {
        if (typeof node[i] === "number" && !isFinite(node[i])) node[i] = null;
        else nullNonFinite(node[i]);
      }
    } else if (node !== null && typeof node === "object") {
      Object.keys(node).forEach(function (key) {
        if (typeof node[key] === "number" && !isFinite(node[key])) node[key] = null;
        else nullNonFinite(node[key]);
      });
    }
  }

  // 變動清單：舊新 stat_data 逐葉比對（路徑是分段陣列）；超過條數就截斷，單筆值太大只記路徑
  function changeValue(entry, field, value) {
    if (value === undefined) return;
    var text = jsonOrString(value);
    if (utf8Length(text) > ZOD_LIMITS.CHANGE_VALUE_BYTES) entry.truncated = true;
    else entry[field] = value;
  }
  function diffChanges(before, after) {
    var changes = [];
    var truncated = false;
    function container(value) {
      return value !== null && typeof value === "object";
    }
    function walk(path, a, b) {
      if (truncated) return;
      if (container(a) && container(b) && Array.isArray(a) === Array.isArray(b)) {
        var keys = lodash.union(Object.keys(a), Object.keys(b));
        for (var i = 0; i < keys.length; i++) {
          var key = keys[i];
          var inA = hasOwn.call(a, key);
          var inB = hasOwn.call(b, key);
          walk(path.concat([key]), inA ? a[key] : undefined, inB ? b[key] : undefined);
        }
        return;
      }
      if (lodash.isEqual(a, b)) return;
      if (changes.length >= ZOD_LIMITS.CHANGES) {
        truncated = true;
        return;
      }
      var entry = { path: path };
      changeValue(entry, "before", a);
      changeValue(entry, "after", b);
      changes.push(entry);
    }
    walk([], before, after);
    return { changes: changes, truncated: truncated };
  }
`;
