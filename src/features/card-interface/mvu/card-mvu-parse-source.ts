// 卡片沙盒 MVU 墊片的 parseMessage 片段（原始碼字串，嵌在 card-mvu-shim-source.ts 的閉包裡）：用到閉包的 TOKEN、
// parentRef、mvuCore、emit、EVENTS、isPlainObject，定義 parseMessage 與 resolveParse。片段規則同共用核心：不得出現
// 反引號與「錢字號加左大括號」。
export const MVU_PARSE_SOURCE = String.raw`
  // zod 桌的 parseMessage（card-shim-no-zod）：交宿主的 zod 引擎只算不寫，與處理 AI 回覆的是同一台（上游 Mvu 掛在酒館
  // 主頁，前端呼叫的也是同一台）。宿主回 zod:false＝確認不是 zod 桌，照舊在這裡跑核心；判定失敗、整次失敗一律 reject，
  // 不退回 MagVarUpdate 語意。同一支 iframe 同時只送一筆（宿主引擎名額少，卡片狂叫會排在 GM 回合後面），10 秒沒回就
  // reject，之後晚到的回覆不認。事件只補通知：STARTED（舊表）→ ENDED → ENDED_for_zod（最終表, 舊表），都給拷貝（監聽器改不到結果）
  var PARSE_HOST_MS = 10000;
  var parseWaiting = new Map();
  var parseSequence = 0;
  var parseSlot = Promise.resolve();
  function resolveParse(data) {
    var done = parseWaiting.get(data.requestId);
    if (!done) return;
    parseWaiting.delete(data.requestId);
    done(data);
  }
  function parseError(code, message) {
    var error = new Error("parseMessage: " + message);
    error.code = code;
    return error;
  }
  // 排隊拿名額：回傳釋放函式
  function acquireParse() {
    var release;
    var mine = new Promise(function (resolve) {
      release = resolve;
    });
    var before = parseSlot;
    parseSlot = before.then(function () {
      return mine;
    });
    return before.then(function () {
      return release;
    });
  }
  function askHost(message, oldText) {
    return new Promise(function (resolve) {
      parseSequence += 1;
      var id = TOKEN + ":p" + String(parseSequence);
      var timer = setTimeout(function () {
        if (!parseWaiting.has(id)) return;
        parseWaiting.delete(id);
        resolve({ ok: false, error: "timeout" });
      }, PARSE_HOST_MS);
      parseWaiting.set(id, function (data) {
        clearTimeout(timer);
        resolve(data);
      });
      parentRef.postMessage(
        { source: "table-tavern-card", kind: "mvu-parse", token: TOKEN, requestId: id, message: message, old: oldText },
        "*"
      );
    });
  }
  async function parseMessage(message, oldData) {
    if (typeof message !== "string") throw new Error("parseMessage: message 必須是字串");
    if (!isPlainObject(oldData)) throw new Error("parseMessage: old_data 必須是物件");
    var utf8 = function (text) {
      return new TextEncoder().encode(text).length;
    };
    if (utf8(message) > mvuCore.limits.MESSAGE_BYTES) throw parseError("message-too-large", "訊息超過 256 KB");
    var oldText;
    try {
      oldText = JSON.stringify(oldData);
    } catch (error) {
      throw parseError("invalid-value", "old_data 存不成 JSON");
    }
    if (utf8(oldText) > mvuCore.limits.TABLE_BYTES) throw parseError("too-large", "old_data 超過 2 MB");
    var release = await acquireParse();
    var reply;
    try {
      reply = await askHost(message, oldText);
    } finally {
      // 先放名額再補事件：監聽器裡再呼叫 parseMessage 才不會互等
      release();
    }
    if (!reply || reply.ok !== true) {
      var code = reply && typeof reply.error === "string" ? reply.error : "host-error";
      throw parseError(code === "timeout" ? "timeout" : "host-failed", "宿主沒有算出結果（" + code.slice(0, 300) + "）");
    }
    if (reply.zod === false) return mvuCore.parseMessage(message, oldData);
    var table;
    try {
      table = typeof reply.table === "string" ? JSON.parse(reply.table) : null;
    } catch (error) {
      table = null;
    }
    if (!isPlainObject(table)) throw parseError("host-failed", "宿主回的結果不是物件");
    // 同核心：STARTED 帶更新前的表；ENDED 系列帶 (新表, 舊表)。舊表是送宿主前那一份（oldText），每支監聽器各拿拷貝
    var before = JSON.parse(oldText);
    await emit(EVENTS.VARIABLE_UPDATE_STARTED, [before], true);
    await emit(EVENTS.VARIABLE_UPDATE_ENDED, [table, before], true);
    await emit(EVENTS.VARIABLE_UPDATE_ENDED + "_for_zod", [table, before], true);
    return table;
  }
`;
