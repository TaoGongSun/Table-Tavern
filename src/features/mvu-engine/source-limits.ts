// MVU 共用核心片段：上限、值解析橋接（走 host）、共用小工具。片段規則與串接順序見 engine-source.ts。
export const SOURCE_LIMITS = String.raw`
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
    return host.utf8Length(text);
  }

  // 值解析請求：交宿主（host.evaluate）、等結果。宿主有計時器（host.timers，iframe、網頁）就再設一道保險，宿主沒回也會
  // 結束；沒有計時器的宿主（QuickJS）由整批期限中斷。保險先設、再送請求；逾時後晚到的結果不認。宿主送請求時拋錯
  // 就照拋（收掉保險）；取消函式拋錯也照樣以逾時結算
  function evalRemote(ctx, op, text) {
    checkDeadline(ctx);
    var timers = host.timers || null;
    return new Promise(function (resolve, reject) {
      var settled = false;
      var cancel = null;
      var guard = timers
        ? timers.set(function () {
            if (settled) return;
            settled = true;
            try {
              if (cancel) cancel();
            } finally {
              reject(parseFailure("eval-timeout", "宿主沒有回應值解析請求"));
            }
          }, Math.max(1000, ctx.deadline - Date.now() + 500))
        : null;
      try {
        cancel = host.evaluate(op, text, function (data) {
          if (settled) return;
          settled = true;
          if (timers) timers.clear(guard);
          resolve(data.ok === true ? { ok: true, value: data.value } : { ok: false, error: String(data.error) });
        });
      } catch (error) {
        if (settled) return;
        settled = true;
        if (timers) timers.clear(guard);
        reject(error);
      }
    });
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

`;
