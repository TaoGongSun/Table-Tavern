// 卡片介面沙盒的讀訊息墊片：getChatMessages／getCurrentMessageId／getLastMessageId 的本場讀取支援。
// 行為對照酒館助手（JS-Slash-Runner）的型別規格與實作行為，只當規格書讀、不抄碼
// （見 .ai/plans/card-chat-messages-shim.md）。資料只進不出：沙盒讀得到本場逐字稿，寫不回 app。
// 不依賴桌面版的事件型別與語系，網頁版也直接共用。

export type ChatRole = "user" | "assistant" | "system";

/** 一樓；樓號＝在 floors 裡的位置 */
export interface ChatFloor {
  name: string;
  role: ChatRole;
  message: string;
}

/** 產生目前殼的那一樓 */
export interface CurrentFloor {
  id: number;
  name: string;
  /** 這一樓交給卡片的原文；骨架路徑是填值後的合成文字 */
  text: string;
}

/** 交給沙盒的本場快照 */
export interface CardChat {
  currentId: number;
  floors: ChatFloor[];
}

/**
 * 掛載與推送共用的快照：floors 是選路算好的每一樓（本樓與歷史樓同一份文字），空桌時開場白就是第 0 樓。
 */
export function buildCardChat(floors: ChatFloor[], current: CurrentFloor): CardChat {
  return {
    currentId: current.id,
    floors: floors.length > 0 ? floors : [{ name: current.name, role: "assistant", message: current.text }],
  };
}

/** 嵌進 script 的 JS 字面值：跳脫 `<`、`>`、`&`、U+2028、U+2029，卡片文字關不掉 `</script>` */
export function scriptLiteral(value: unknown): string {
  return JSON.stringify(value)
    .replace(/</g, "\\u003c")
    .replace(/>/g, "\\u003e")
    .replace(/&/g, "\\u0026")
    .replace(/\u2028/g, "\\u2028")
    .replace(/\u2029/g, "\\u2029");
}

/**
 * 沙盒裡的三支讀訊息函式。必須排在任何覆寫 window.parent 的墊片與卡片 script 之前執行：
 * 真 parent 在這裡先收進閉包，之後只認它推來、token 相符、形狀正確的 chat 快照。
 */
export function buildChatShimSource(chat: CardChat, token: string): string {
  return `
(function () {
  var parentRef = window.parent;
  var TOKEN = ${scriptLiteral(token)};
  var store = ${scriptLiteral(chat)};
  var ROLES = { all: true, system: true, assistant: true, user: true };
  var HIDE_STATES = { all: true, hidden: true, unhidden: true };

  function copy(value) {
    return JSON.parse(JSON.stringify(value));
  }

  function validChat(value) {
    if (!value || typeof value !== "object" || !Array.isArray(value.floors)) return false;
    if (!Number.isInteger(value.currentId)) return false;
    for (var i = 0; i < value.floors.length; i++) {
      var floor = value.floors[i];
      if (!floor || typeof floor !== "object") return false;
      if (typeof floor.name !== "string" || typeof floor.message !== "string") return false;
      if (floor.role !== "user" && floor.role !== "assistant" && floor.role !== "system") return false;
    }
    return true;
  }

  window.addEventListener("message", function (event) {
    if (event.source !== parentRef) return;
    var data = event.data;
    if (!data || data.source !== "table-tavern-host" || data.kind !== "chat" || data.token !== TOKEN) return;
    if (!validChat(data.chat)) return;
    store = copy(data.chat);
  });

  // 格式已由正規式保證是整數字串；超長的會變成 ±Infinity，交給後面的夾回處理（正數落最後一樓、負數落第 0 樓）
  function toIndex(text, last) {
    var value = Number(text);
    return value < 0 ? last + value + 1 : value;
  }

  // 酒館行為：range 轉字串、代換 {{lastMessageId}}，認「單一樓號」或「a-b」（兩端都可為負數深度），
  // 反序自動排序、兩端夾進現有樓層；其他格式回空陣列。
  function parseRange(range, last) {
    var text = String(range).replace(/\\{\\{lastMessageId\\}\\}/g, String(last));
    var single = /^(-?\\d+)$/.exec(text);
    var pair = single ? null : /^(-?\\d+)-(-?\\d+)$/.exec(text);
    if (!single && !pair) return null;
    var start = toIndex(single ? single[1] : pair[1], last);
    var end = toIndex(single ? single[1] : pair[2], last);
    if (start > end) {
      var swap = start;
      start = end;
      end = swap;
    }
    return [Math.min(Math.max(start, 0), last), Math.min(Math.max(end, 0), last)];
  }

  function shape(id, floor, withSwipes) {
    var base = { message_id: id, name: floor.name, role: floor.role, is_hidden: false };
    if (withSwipes) {
      base.swipe_id = 0;
      base.swipes = [floor.message];
      base.swipes_data = [{}];
      base.swipes_info = [{}];
      return base;
    }
    base.message = floor.message;
    base.data = {};
    base.extra = {};
    base.swipe_id = 0;
    base.swipes = [floor.message];
    base.swipes_data = [{}];
    return base;
  }

  window.getChatMessages = function (range, option) {
    // 酒館第一步就把 range 轉字串，null／undefined 在那裡拋 TypeError
    if (range === null || range === undefined) {
      throw new TypeError("getChatMessages: range 不得為 " + String(range));
    }
    var floors = store.floors;
    var last = floors.length - 1;
    if (last < 0) return [];
    var options = option && typeof option === "object" ? option : {};
    var role = options.role === undefined ? "all" : options.role;
    var hideState = options.hide_state === undefined ? "all" : options.hide_state;
    if (!ROLES[role] || !HIDE_STATES[hideState]) return [];
    var bounds = parseRange(range, last);
    if (!bounds) return [];
    var result = [];
    for (var id = bounds[0]; id <= bounds[1]; id++) {
      var floor = floors[id];
      if (role !== "all" && floor.role !== role) continue;
      if (hideState === "hidden") continue;
      result.push(shape(id, floor, options.include_swipes === true));
    }
    return copy(result);
  };

  window.getCurrentMessageId = function () {
    return store.currentId;
  };

  window.getLastMessageId = function () {
    return store.floors.length - 1;
  };
})();
`;
}
