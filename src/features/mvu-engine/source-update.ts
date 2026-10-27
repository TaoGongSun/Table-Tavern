// MVU 共用核心片段：updateVariables 事件分階段、結果上限、parseMessage。片段規則與串接順序見 engine-source.ts。
export const SOURCE_UPDATE = String.raw`
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
    // zod 卡（mvu_zod）：上游是卡片腳本載入時就註冊的監聽器，排在其他監聽器之前；逐條套用後清空指令，
    // 下面的 MagVarUpdate 套用迴圈就不跑
    var zod = zodActive();
    if (zod) await zodCommandParsed(ctx, variables, commands);
    await emit(EVENTS.COMMAND_PARSED + "_for_zod", [variables, commands, content], false);
    if (zod) commands.length = 0;
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
        ownSet(outStatus.stat_data, path, display);
        ownSet(deltaStatus.stat_data, path, display);
      }
    }
    variables.display_data = outStatus.stat_data;
    variables.delta_data = deltaStatus.stat_data;
    await emit(EVENTS.VARIABLE_UPDATE_ENDED, [variables, before], false);
    checkDeadline(ctx);
    lodash.unset(variables.stat_data, "$internal");
    var modified = !lodash.isEqual(variables.stat_data, before.stat_data);
    if (modified) reconcileAndApplySchema(variables);
    if (zod) {
      nullNonFinite(variables.stat_data);
      zodUpdateEnded(variables);
    }
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

  // set 產生、到整批結束都還是非有限數的位置轉成 null（同上游存檔時的 JSON.stringify）；其餘來源的非有限數照舊由上限擋下
  function nullNonFiniteSets(ctx, result) {
    ctx.nonFiniteSets.forEach(function (entry) {
      var holder = entry.path === "" ? result : ownGet(result.stat_data, entry.path);
      if (entry.vwd) {
        if (Array.isArray(holder) && typeof holder[0] === "number" && !isFinite(holder[0])) holder[0] = null;
      } else if (typeof holder === "number" && !isFinite(holder)) {
        if (entry.path === "") result.stat_data = null;
        else ownSet(result.stat_data, entry.path, null);
      }
    });
  }

  // 整次處理：深拷貝 old、照訊息更新、套上限；回結果與這次的 ctx（zod 逐條結果在 ctx.zodResults）
  async function runParse(message, oldData) {
    if (typeof message !== "string") throw new Error("parseMessage: message 必須是字串");
    if (!isPlainObject(oldData)) throw new Error("parseMessage: old_data 必須是物件");
    if (utf8Length(message) > PARSE_LIMITS.MESSAGE_BYTES) throw parseFailure("message-too-large", "訊息超過 256 KB");
    var ctx = { deadline: Date.now() + PARSE_LIMITS.TOTAL_MS, nonFiniteSets: [], zodResults: [] };
    var result = lodash.cloneDeep(oldData);
    await updateVariables(ctx, message, result);
    checkDeadline(ctx);
    nullNonFiniteSets(ctx, result);
    var problem = limitProblem(result);
    if (problem) throw parseFailure(problem, "結果超出上限（" + problem + "）");
    return { result: result, ctx: ctx };
  }

  // Mvu.parseMessage(message, old)：深拷貝 old 後照訊息更新、回新資料（上游一律回新資料，沒有變動也是）。
  // 純沙盒運算，不送宿主寫入、不落檔、不記帳；超過上限（訊息 256 KB、指令 1000 條、整次 5 秒、結果同 8.8）丟錯
  async function parseMessage(message, oldData) {
    return (await runParse(message, oldData)).result;
  }

  // 宿主提交用（mvu-zod-semantics 回傳契約）：完整新表＋zod 逐條結果＋stat_data 逐葉變動。整次失敗照樣丟錯
  async function parseMessageDetailed(message, oldData) {
    var run = await runParse(message, oldData);
    var diff = diffChanges(oldData.stat_data, run.result.stat_data);
    return {
      table: run.result,
      zod: zodActive(),
      results: run.ctx.zodResults,
      changes: diff.changes,
      changesTruncated: diff.truncated
    };
  }
`;
