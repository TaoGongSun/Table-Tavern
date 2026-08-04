// ST 角色卡的「顯示用 regex 腳本」轉換層：把模型輸出套上卡片自帶的 regex，
// 抽出內嵌的整頁 HTML 介面，再組成可直接餵給沙盒 iframe srcdoc 的文件。

export interface InterfaceScript {
  name: string;
  find_regex: string;
  replace_string: string;
  trim_strings: string[];
  min_depth: number | null;
  max_depth: number | null;
}

/** 後端 `card_interfaces` 回傳的一張卡；`unsupported` 非 null＝DRM 卡或雲端載入器卡，畫不出來。 */
export interface CardInterface {
  character_id: string;
  character_name: string;
  scripts: InterfaceScript[];
  unsupported: string | null;
  opening: string | null;
}

/**
 * 從幾段候選文字裡挑出第一個畫得出來的殼（依序試，先命中先用）。
 * 面板與匯入流程共用同一套判斷，才不會出現「按鈕說有、打開卻空的」。
 */
export function findShell(cards: CardInterface[], texts: (string | null | undefined)[]): string | null {
  const scripts = cards.filter((card) => card.unsupported === null).flatMap((card) => card.scripts);
  if (scripts.length === 0) return null;
  for (const text of texts) {
    if (!text) continue;
    const shell = extractShell(applyScripts(text, scripts));
    if (shell !== null) return shell;
  }
  return null;
}

// JS 認得的旗標；ST 卡常常寫上 JS 沒有的旗標（例如 ST 自家的擴充），直接丟掉即可。
const VALID_JS_FLAGS = new Set(["d", "g", "i", "m", "s", "u", "v", "y"]);

/**
 * ST 的 findRegex 有兩種寫法：`/樣式/旗標` 與裸樣式。壞樣式回 null，絕不丟例外。
 */
export function parseStRegex(findRegex: string): RegExp | null {
  let pattern = findRegex;
  let rawFlags = "";

  if (findRegex.startsWith("/")) {
    const lastSlash = findRegex.lastIndexOf("/");
    if (lastSlash > 0) {
      pattern = findRegex.slice(1, lastSlash);
      rawFlags = findRegex.slice(lastSlash + 1);
    }
  }

  const flags = Array.from(new Set(rawFlags.split("").filter((flag) => VALID_JS_FLAGS.has(flag)))).join("");

  try {
    return new RegExp(pattern, flags);
  } catch {
    return null;
  }
}

// replace_string 裡的代換只做一輪：掃過去遇到 {{match}}／$1..$9 就換，其餘原樣輸出。
// 不能先換 {{match}} 再對結果跑一次 $1 代換，命中文字裡若剛好含 $1 會被二次代換。
function substitute(template: string, match: string, groups: (string | undefined)[]): string {
  let result = "";
  for (let i = 0; i < template.length; i++) {
    if (template.startsWith("{{match}}", i)) {
      result += match;
      i += "{{match}}".length - 1;
      continue;
    }
    const char = template[i];
    const next = template[i + 1];
    if (char === "$" && next !== undefined && next >= "1" && next <= "9") {
      result += groups[Number(next) - 1] ?? "";
      i += 1;
      continue;
    }
    result += char;
  }
  return result;
}

function trimAll(value: string, trimStrings: string[]): string {
  return trimStrings.reduce((acc, needle) => (needle ? acc.split(needle).join("") : acc), value);
}

/**
 * 依序套用顯示腳本；單支炸掉就跳過該支續走下一支，永遠回傳字串。
 */
export function applyScripts(raw: string, scripts: InterfaceScript[]): string {
  let text = raw;

  for (const script of scripts) {
    // min_depth／max_depth 本期不理會：我們只渲染最新一則訊息，深度恆為 0。
    try {
      const regex = parseStRegex(script.find_regex);
      if (!regex) continue;

      text = text.replace(regex, (match: string, ...rest: unknown[]) => {
        const hasNamedGroups = rest.length > 0 && typeof rest[rest.length - 1] === "object";
        const positional = hasNamedGroups ? rest.slice(0, -1) : rest;
        const captures = positional.slice(0, -2) as (string | undefined)[];

        const trimmedMatch = trimAll(match, script.trim_strings);
        const trimmedCaptures = captures.map((group) => (group === undefined ? undefined : trimAll(group, script.trim_strings)));

        return substitute(script.replace_string, trimmedMatch, trimmedCaptures);
      });
    } catch {
      // 這支腳本壞掉：保留前一步的結果，續走下一支。
    }
  }

  return text;
}

const FENCE_REGEX = /```([a-zA-Z]*)\r?\n([\s\S]*?)```/g;
const SHELL_START_MARKERS = /^\s*(<!DOCTYPE|<html|<body)/i;
const BARE_SHELL_MARKER = /<!DOCTYPE html|<html/i;

/**
 * 抽出卡片內嵌的整頁 HTML；抽不到回 null。
 */
export function extractShell(rendered: string): string | null {
  const parts: string[] = [];
  FENCE_REGEX.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = FENCE_REGEX.exec(rendered)) !== null) {
    const lang = match[1].toLowerCase();
    const content = match[2];
    if (lang === "html" || (lang === "" && SHELL_START_MARKERS.test(content))) {
      parts.push(content);
    }
  }

  if (parts.length > 0) {
    const joined = parts.join("\n").trim();
    return joined.length > 0 ? joined : null;
  }

  const bare = BARE_SHELL_MARKER.exec(rendered);
  if (bare) {
    const tail = rendered.slice(bare.index).trim();
    return tail.length > 0 ? tail : null;
  }

  return null;
}

// 宿主橋接墊片：沙盒 iframe 是 allow-scripts、沒有 allow-same-origin，碰不到宿主 DOM，
// 所以在 iframe 內偽造一個誘餌輸入框，把卡片戳 window.parent/window.top 的動作攔下來轉成 postMessage。
function buildHostBridgeShim(): string {
  return `<script>
(function () {
  var parentRef = window.parent;

  // 標記這則是不是玩家真的動手點出來的。卡片自己觸發一回合是 ST 上的正常用法（照送），
  // 但我們每收到新訊息就重載整個 iframe，「載入就送」會變成無限迴圈——宿主靠這個旗標踩煞車。
  var lastGesture = 0;
  ["click", "keydown", "touchend"].forEach(function (type) {
    document.addEventListener(type, function (event) { if (event.isTrusted) lastGesture = Date.now(); }, true);
  });

  function notifyHost(text) {
    try {
      parentRef.postMessage({
        source: "table-tavern-card",
        kind: "input",
        text: text,
        gesture: Date.now() - lastGesture < 1500,
      }, "*");
    } catch (error) {
      console.warn("[table-tavern] 無法送出訊息給宿主", error);
    }
  }

  var bait = document.createElement("textarea");
  bait.id = "send_textarea";
  bait.style.display = "none";
  bait.addEventListener("input", function () {
    notifyHost(bait.value);
    // 送出後立刻清空：卡片常寫成「舊值有東西就接在後面」，不清會讓連點兩個行動黏成一串。
    bait.value = "";
  });
  function mountBait() {
    document.body.appendChild(bait);
  }
  if (document.body) {
    mountBait();
  } else {
    document.addEventListener("DOMContentLoaded", mountBait);
  }

  // 各卡自備一串輸入框選擇器輪流試（真卡實測有 #send_textarea、textarea#send_textarea、
  // #chat-input、#user-input、#prompt-textarea 等），一律導到誘餌；認不得的回 null 讓卡片自己退路。
  var INPUT_HINT = /textarea|send_textarea|chatinput|chat-input|user-input|prompt-textarea/i;
  function getBaitById(id) {
    return typeof id === "string" && INPUT_HINT.test(id) ? bait : null;
  }
  function getBaitBySelector(selector) {
    return typeof selector === "string" && INPUT_HINT.test(selector) ? bait : null;
  }

  window.__ttHost = {
    document: {
      getElementById: getBaitById,
      querySelector: getBaitBySelector,
      querySelectorAll: function (selector) {
        var found = getBaitBySelector(selector);
        return found ? [found] : [];
      },
    },
  };

  window.triggerSlash = function (command) {
    if (typeof command !== "string") {
      console.warn("[table-tavern] triggerSlash 收到非字串指令", command);
      return;
    }
    if (command.indexOf("/send") === 0) {
      var text = command.slice(5).replace(/^\\s+/, "");
      var pipeIndex = text.indexOf("|");
      notifyHost(pipeIndex >= 0 ? text.slice(0, pipeIndex) : text);
      return;
    }
    if (command.indexOf("/trigger") === 0) {
      notifyHost("");
      return;
    }
    console.warn("[table-tavern] 不認得的 triggerSlash 指令", command);
  };

  try {
    Object.defineProperty(window, "parent", { value: window.__ttHost, configurable: true });
  } catch (error) {
    // WKWebView／WebView2 能不能覆寫還沒實測，失敗不影響其餘墊片。
  }
  try {
    Object.defineProperty(window, "top", { value: window.__ttHost, configurable: true });
  } catch (error) {
    // 同上。
  }
})();
</script>`;
}

/**
 * 把殼包成可直接餵給沙盒 iframe srcdoc 的完整文件（含宿主橋接墊片）。
 */
export function buildShellDocument(shell: string): string {
  const shim = buildHostBridgeShim();
  // 墊片攔不到的備案：把殼原始碼裡完整字面的 window.parent／window.top 直接改指向誘餌，
  // \b 確保只換完整字面，不誤傷 window.parentNode 或 node.parent.foo 這類正常寫法。
  const processedShell = shell.replace(/window\.parent\b/g, "window.__ttHost").replace(/window\.top\b/g, "window.__ttHost");

  const headMatch = /<head[^>]*>/i.exec(processedShell);
  if (headMatch) {
    const insertAt = headMatch.index + headMatch[0].length;
    return processedShell.slice(0, insertAt) + shim + processedShell.slice(insertAt);
  }

  const bodyMatch = /<body[^>]*>/i.exec(processedShell);
  if (bodyMatch) {
    const insertAt = bodyMatch.index + bodyMatch[0].length;
    return processedShell.slice(0, insertAt) + shim + processedShell.slice(insertAt);
  }

  return `<!DOCTYPE html><html><head>${shim}</head><body>${processedShell}</body></html>`;
}
