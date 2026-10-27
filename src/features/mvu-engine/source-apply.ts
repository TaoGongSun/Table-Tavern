// MVU 共用核心片段：套用指令：set／insert／delete／add 與非有限數標記。片段規則與串接順序見 engine-source.ts。
export const SOURCE_APPLY = String.raw`
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

  // 寫入 path（有 key 就是 path 底下那一鍵）：撤銷跟它重疊（同一處、祖先、子孫）的「set 產生的非有限數」標記——
  // 那個位置最後一次寫入的來源已經不是那條 set（作者裁決 2026-10-04：只有 set 產生、到整批結束都沒被改寫的才轉 null）
  function touch(st, path, key) {
    var segments = lodash.toPath(path);
    if (key !== undefined) segments = segments.concat([String(key)]);
    touchSegments(st, segments);
  }
  function touchSegments(st, segments) {
    st.ctx.nonFiniteSets = st.ctx.nonFiniteSets.filter(function (entry) {
      var shared = Math.min(entry.segments.length, segments.length);
      for (var i = 0; i < shared; i++) if (entry.segments[i] !== segments[i]) return true;
      return false;
    });
  }

  // splice 只移動位置、不改寫元素值：set 產生的非有限數標記跟著位移，被刪掉的那一項的標記丟掉；只動被 splice
  // 那一層容器的直接索引段〔主線裁定，依作者裁決 2026-10-04 非有限數轉 null；後端 array.rs 同步〕
  function shiftSets(st, container, index, step) {
    var depth = container.length;
    st.ctx.nonFiniteSets = st.ctx.nonFiniteSets.filter(function (entry) {
      if (entry.segments.length <= depth) return true;
      for (var i = 0; i < depth; i++) if (entry.segments[i] !== String(container[i])) return true;
      if (!/^\d+$/.test(entry.segments[depth])) return true;
      var at = Number(entry.segments[depth]);
      if (step < 0 && at === index) return false;
      if (step > 0 ? at >= index : at > index) {
        entry.segments = entry.segments.slice();
        entry.segments[depth] = String(at + step);
        entry.path = segmentsToPath(entry.segments);
      }
      return true;
    });
  }
  // Array.prototype.splice 的起點：負數從尾端算、超出範圍夾在 0..長度
  function spliceAt(index, length) {
    if (index < 0) return Math.max(length + index, 0);
    return Math.min(index, length);
  }

  // lodash.merge(target, source) 實際會覆寫的位置才撤銷：兩邊都是可合併的容器就往下遞迴（照 lodash：來源陣列只合進
  // 陣列、來源物件合進任何物件），否則整個鍵被換掉；來源是 undefined 的鍵不寫。沒被動到的兄弟欄保留標記
  function touchMerge(st, segments, source, target) {
    Object.keys(source).forEach(function (key) {
      var from = source[key];
      if (from === undefined) return;
      var here = segments.concat([key]);
      var into = target !== null && typeof target === "object" ? target[key] : undefined;
      var deeper = Array.isArray(from)
        ? Array.isArray(into)
        : lodash.isPlainObject(from) && into !== null && typeof into === "object";
      if (deeper) touchMerge(st, here, from, into);
      else touchSegments(st, here);
    });
  }

  async function applySet(st, command, path, reasonStr) {
    var variables = st.variables;
    if (path !== "" && !ownHas(variables.stat_data, path)) {
      outError(st, "set 的路徑不存在：" + path + " " + reasonStr);
      return "";
    }
    var oldValue = path === "" ? lodash.cloneDeep(variables.stat_data) : ownGet(variables.stat_data, path);
    var newValue = isoIfDate(await parseValue(st.ctx, command.args[command.args.length - 1]));
    var pathIsVwd = false;
    touch(st, path);
    // 偏離上游：新值是陣列時整個換掉，不寫進第 0 項（上游會巢狀成 [[…], 說明]，把恰好兩個字串的清單寫壞）
    // 〔主線裁定，依作者裁決 酒館行為照做；後端 upstream_set 同步〕
    var wholeVwd = !st.strictSet && isVwd(oldValue) && !Array.isArray(oldValue[0]) && Array.isArray(newValue);
    if (!st.strictSet && isVwd(oldValue) && !Array.isArray(oldValue[0]) && !wholeVwd) {
      // [值, 說明]：改第 0 項；舊值是數字且新值不是 null 才強轉成數字（數字欄位可以設成 null）
      var oldCopy = lodash.cloneDeep(oldValue[0]);
      oldValue[0] = typeof oldValue[0] === "number" && newValue !== null ? Number(newValue) : newValue;
      oldValue = oldCopy;
      pathIsVwd = true;
    } else if (typeof oldValue === "number" && newValue !== null && typeof newValue === "string") {
      ownSet(variables.stat_data, path, Number(newValue));
    } else if (path) {
      ownSet(variables.stat_data, path, newValue);
    } else {
      variables.stat_data = newValue;
    }
    var finalValue = path === "" ? variables.stat_data : ownGet(variables.stat_data, path);
    if (pathIsVwd) finalValue = finalValue[0];
    // set 轉出的非有限數（Number("很多") 等）照上游留在記憶體裡，整批做完、序列化前才轉 null（作者裁決 2026-10-04）
    if (typeof finalValue === "number" && !isFinite(finalValue)) {
      st.ctx.nonFiniteSets.push({ path: path, segments: lodash.toPath(path), vwd: pathIsVwd });
    }
    var display;
    if (!st.strictSet && !wholeVwd && isVwd(oldValue) && Array.isArray(finalValue)) {
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
    var existing = path === "" ? variables.stat_data : ownGet(variables.stat_data, path);
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
    } else if (
      // 偏離上游：父路徑是根（頂層容器，如 /物品/-）時上游的 lodash.get(資料, "") 恆為 undefined 而擋下，
      // GM 最常寫的寫法會失敗，這裡放行〔主線裁定，依作者裁決 酒館行為照做〕
      lodash.toPath(path).length > 1 &&
      !ownGet(variables.stat_data, segmentsToPath(lodash.toPath(path).slice(0, -1)))
    ) {
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
      collection = path === "" ? variables.stat_data : ownGet(variables.stat_data, path);
      if (!Array.isArray(collection) && !lodash.isObject(collection)) {
        collection = Array.isArray(valueToAssign) ? [] : {};
        touch(st, path);
        ownSet(variables.stat_data, path, collection);
      }
      if (Array.isArray(collection)) {
        template = targetSchema && isArraySchema(targetSchema) ? targetSchema.template : undefined;
        valueToAssign = applyTemplate(valueToAssign, template, st.strictTemplate, st.concatTemplateArray);
        touch(st, path, collection.length);
        collection.push(valueToAssign);
        display = "ASSIGNED " + JSON.stringify(valueToAssign) + " into array '" + path + "' " + reasonStr;
        successful = true;
      } else if (lodash.isObject(collection)) {
        if (lodash.isObject(valueToAssign) && !Array.isArray(valueToAssign)) {
          touchMerge(st, lodash.toPath(path), valueToAssign, collection);
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
      collection = path === "" ? variables.stat_data : ownGet(variables.stat_data, path);
      template = targetSchema && (isArraySchema(targetSchema) || isObjectSchema(targetSchema)) ? targetSchema.template : undefined;
      if (Array.isArray(collection) && (typeof keyOrIndex === "number" || keyOrIndex === "-")) {
        // JSON Patch 的 "-" 是追加到陣列尾
        var index = keyOrIndex === "-" ? collection.length : keyOrIndex;
        shiftSets(st, lodash.toPath(path), spliceAt(index, collection.length), 1);
        var label = keyOrIndex === "-" || keyOrIndex === -1 ? "tail" : keyOrIndex;
        valueToAssign = applyTemplate(valueToAssign, template, st.strictTemplate, st.concatTemplateArray);
        collection.splice(index, 0, valueToAssign);
        display = "ASSIGNED " + JSON.stringify(valueToAssign) + " into '" + path + "' at index " + label + " " + reasonStr;
        successful = true;
      } else if (lodash.isObject(collection)) {
        valueToAssign = applyTemplate(valueToAssign, template, st.strictTemplate, st.concatTemplateArray);
        touch(st, path, keyOrIndex);
        setOwn(collection, String(keyOrIndex), valueToAssign);
        display = "ASSIGNED key '" + keyOrIndex + "' with value " + JSON.stringify(valueToAssign) + " into object '" + path + "' " + reasonStr;
        successful = true;
      } else {
        collection = {};
        touch(st, path);
        ownSet(variables.stat_data, path, collection);
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
    var newValue = isNullOrWhiteSpace(path) ? variables.stat_data : ownGet(variables.stat_data, path);
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
      var container = ownGet(variables.stat_data, arrayPath);
      var at = parseInt(lastPart, 10);
      if (Array.isArray(container) && at < container.length) {
        var originalArray = lodash.cloneDeep(container);
        shiftSets(st, lodash.toPath(arrayPath), at, -1);
        container.splice(at, 1);
        await singleUpdated(st, arrayPath, originalArray, container);
        return "";
      }
    }
    if (!ownHas(variables.stat_data, path)) {
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
    if (containerPath !== "" && !ownHas(variables.stat_data, containerPath)) {
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
      var oldValue = ownGet(variables.stat_data, path);
      touch(st, path);
      ownUnset(variables.stat_data, path);
      display = "REMOVED path '" + path + "' " + reasonStr;
      removed = true;
      await singleUpdated(st, path, oldValue, undefined);
    } else {
      var collection = ownGet(variables.stat_data, path);
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
          touch(st, path);
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
            touch(st, path, byIndex);
            ownUnset(collection, byIndex);
            removed = true;
            display = "REMOVED " + (target + 1) + "th entry ('" + byIndex + "') from object '" + path + "' " + reasonStr;
          }
        } else {
          var byName = String(target);
          if (ownHas(collection, byName)) {
            touch(st, path, byName);
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
    if (!ownHas(variables.stat_data, path)) {
      outError(st, "add 的路徑不存在：" + path + " " + reasonStr);
      return "";
    }
    var initial = lodash.cloneDeep(ownGet(variables.stat_data, path));
    var oldValue = ownGet(variables.stat_data, path);
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
      touch(st, path);
      if (vwd) {
        oldValue[0] = shifted;
        ownSet(variables.stat_data, path, oldValue);
      } else {
        ownSet(variables.stat_data, path, shifted);
      }
    } else if (typeof valueToAdd === "number") {
      if (typeof delta !== "number") {
        outError(st, "增量必須是數字：" + command.args[1] + " " + reasonStr);
        return "";
      }
      var sum = parseFloat((valueToAdd + delta).toPrecision(12));
      touch(st, path);
      if (vwd) {
        oldValue[0] = sum;
        ownSet(variables.stat_data, path, oldValue);
      } else {
        ownSet(variables.stat_data, path, sum);
      }
    } else {
      outError(st, "add 不支援這種值：" + path + " " + reasonStr);
      return "";
    }
    finalValue = ownGet(variables.stat_data, path);
    display = vwd
      ? JSON.stringify(initial[0]) + "->" + JSON.stringify(finalValue[0]) + " " + reasonStr
      : JSON.stringify(initial) + "->" + JSON.stringify(finalValue) + " " + reasonStr;
    await singleUpdated(st, path, initial, finalValue);
    return display;
  }

`;
