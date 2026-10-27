// MVU 共用核心片段：指令抽取：JSONPatch 區塊與 _.set 類指令、巨集代換。片段規則與串接順序見 engine-source.ts。
export const SOURCE_COMMANDS = String.raw`
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

  // 等長遮罩：區間內除 CR／LF 外換成空白，偏移與原文一致（UTF-16 碼元）
  function maskSpan(chars, start, end) {
    for (var k = start; k < end; k++) if (chars[k] !== "\n" && chars[k] !== "\r") chars[k] = " ";
  }

  // 思考區（照上游 MagVarUpdate 5cf5685a）：think／thinking／reasoning／analysis／analyze 不分大小寫、可帶屬性；
  // 堆疊只彈同名最近一層，錯配的結束標籤略過，未閉合遮到結尾；區外落單的結束標籤只遮標籤本身
  function maskReasoning(text) {
    var chars = text.split("");
    var tag = /<(\/?)\s*(thinking|think|reasoning|analysis|analyze)\b[^>]*>/gi;
    var stack = [];
    var from = 0;
    var found;
    while ((found = tag.exec(text)) !== null) {
      var name = found[2].toLowerCase();
      if (!found[1]) {
        if (stack.length === 0) from = found.index;
        stack.push(name);
      } else if (stack.length === 0) {
        maskSpan(chars, found.index, tag.lastIndex);
      } else if (stack[stack.length - 1] === name) {
        stack.pop();
        if (stack.length === 0) maskSpan(chars, from, tag.lastIndex);
      }
    }
    if (stack.length > 0) maskSpan(chars, from, text.length);
    return chars;
  }

  async function extractCommands(ctx, text) {
    var results = [];
    // 遮罩視圖：先遮思考區，在視圖上找 JSONPatch（思考區裡的不算），再把 JSONPatch 區塊整段遮掉才掃舊式指令，
    // patch 裡看起來像 _.set 的字串是資料、不再跑一次。所有內容都按視圖的區間從原文切
    var view = maskReasoning(text);
    var viewText = view.join("");
    // JSONPatch 區塊：內容交 Worker 解析（YAML／JSON5／修殘缺 JSON），解析失敗的區塊略過
    var patchPattern = /<(json_?patch)>(?:\s*\x60\x60\x60.*)?((?:(?!<json_?patch>)[\s\S])*?)(?:\x60\x60\x60\s*)?<\/\1>/gim;
    var blocks = [];
    var found;
    while ((found = patchPattern.exec(viewText)) !== null) {
      var closeTag = ("</" + found[1] + ">").length;
      var fence = found[0].slice(0, found[0].length - closeTag).match(/\x60\x60\x60\s*$/);
      var bodyEnd = found.index + found[0].length - closeTag - (fence ? fence[0].length : 0);
      blocks.push({ index: found.index, body: text.substring(bodyEnd - found[2].length, bodyEnd).trim() });
      maskSpan(view, found.index, patchPattern.lastIndex);
    }
    for (var b = 0; b < blocks.length; b++) {
      var outcome = await evalRemote(ctx, "patch", blocks[b].body);
      if (outcome.ok && isJsonPatch(outcome.value)) {
        patchToCommands(outcome.value).forEach(function (command) {
          command.$index = blocks[b].index;
          results.push(command);
        });
      }
    }
    var legacyText = view.join("");
    var scan = /_\.(set|insert|assign|remove|unset|delete|add)\(/g;
    var comment = /\s*\/\/(.*)/y;
    var i = 0;
    while (i < text.length) {
      scan.lastIndex = i;
      var match = scan.exec(legacyText);
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
    var macros = host.macros();
    if (!macros) return text;
    return text.replace(/\{\{(user|char)\}\}/gi, function (match, name) {
      if (name.toLowerCase() === "user") return macros.user;
      return macros.char === null || macros.char === undefined ? match : macros.char;
    });
  }

`;
