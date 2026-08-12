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

/** 卡片殼寫在沙盒 localStorage 裡的東西（設定分頁的主題、字級等）；宿主原樣存、原樣回填。 */
export type CardStorage = Record<string, string>;

// 卡片殼能往宿主存的上限。殼只該存設定這種小東西，第三方 JS 不能無限往宿主存檔寫。
export const CARD_STORAGE_LIMIT = 64 * 1024;

/**
 * 把來路不明的值（沙盒 postMessage 過來的、宿主存檔讀回來的）收成乾淨的 CardStorage；
 * 型別不對或整份超過上限回 null，呼叫端當作沒有這份快照。
 */
export function sanitizeCardStorage(value: unknown): CardStorage | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const entries = Object.entries(value).filter(([, item]) => typeof item === "string") as [string, string][];
  const clean = Object.fromEntries(entries);
  return JSON.stringify(clean).length > CARD_STORAGE_LIMIT ? null : clean;
}

/**
 * Storage 墊片原始碼（純 JS，供 shim 內嵌）：沙盒 iframe 沒有 allow-same-origin，origin 是
 * opaque，碰 localStorage／sessionStorage 一律拋 SecurityError。卡片殼常在初始化就讀設定
 * （Vue setup 裡一句 getItem 就讓整支 app 掛不起來，配上 `[v-cloak] { display: none }` 就是整片白），
 * 所以拿不到就換成同介面的記憶體版；拿得到的環境原封不動。
 *
 * localStorage 那份額外接宿主：開場用 seed 回填上次的值，之後每次寫入把整份快照 postMessage
 * 回去存，殼重掛才留得住玩家調過的設定。sessionStorage 照其語意只活在這次掛載，不外送。
 */
export function buildStorageShimSource(seed: CardStorage = {}): string {
  // seed 的值是卡片自己寫的內容，可能含 `</script>`：跳脫 `<` 才不會提前關掉這支 script。
  const seedLiteral = JSON.stringify(seed).replace(/</g, "\\u003c");
  return `
  function installStorageShim(name, seed, persist) {
    try {
      window[name].getItem("table-tavern-probe");
      return;
    } catch (error) {
      // 這個環境給不出 Storage，往下換記憶體版
    }
    var hostRef = window.parent;
    var memory = {};
    for (var seedKey in seed) {
      if (Object.prototype.hasOwnProperty.call(seed, seedKey)) memory[seedKey] = seed[seedKey];
    }
    function sync() {
      if (!persist) return;
      try {
        var payload = JSON.stringify(memory);
        if (payload.length > ${CARD_STORAGE_LIMIT}) {
          console.warn("[table-tavern] 卡片存的設定超過上限，這次不存回宿主");
          return;
        }
        hostRef.postMessage({ source: "table-tavern-card", kind: "storage", entries: JSON.parse(payload) }, "*");
      } catch (error) {
        console.warn("[table-tavern] 無法把卡片設定存回宿主", error);
      }
    }
    var storage = {
      getItem: function (key) {
        return Object.prototype.hasOwnProperty.call(memory, String(key)) ? memory[String(key)] : null;
      },
      setItem: function (key, value) {
        memory[String(key)] = String(value);
        sync();
      },
      removeItem: function (key) {
        delete memory[String(key)];
        sync();
      },
      clear: function () {
        memory = {};
        sync();
      },
      key: function (index) {
        var keys = Object.keys(memory);
        return index < keys.length ? keys[index] : null;
      },
      get length() {
        return Object.keys(memory).length;
      },
    };
    try {
      Object.defineProperty(window, name, { value: storage, configurable: true });
    } catch (error) {
      console.warn("[table-tavern] 無法替換 " + name, error);
    }
  }
  installStorageShim("localStorage", ${seedLiteral}, true);
  installStorageShim("sessionStorage", {}, false);
`;
}

/**
 * IME 守衛原始碼（純 JS，供 shim 內嵌）：卡片殼常自己 listen window 的 keydown，只認
 * `event.key === "Enter"` 就送出——注音／拼音選字那個 Enter 會被當成送出（同一張卡在 ST 上
 * 也這樣）。墊片跑在卡片的 script 之前，先在 capture 階段攔下組字中的按鍵，傳不到卡片的
 * handler；不碰 preventDefault，輸入法照常選字。
 *
 * 攔的是組字中的所有按鍵而不只 Enter：選字窗的方向鍵、數字鍵、取消組字的 Esc 同樣不該被
 * 卡片的快捷鍵吃掉。isComposing 在 WebKit 某些輸入法狀態下不可靠，補看傳統的 keyCode 229。
 */
export function buildImeGuardSource(): string {
  return `
  window.addEventListener(
    "keydown",
    function (event) {
      if (event.isComposing || event.keyCode === 229) event.stopImmediatePropagation();
    },
    true
  );
`;
}

// 宿主橋接墊片：沙盒 iframe 是 allow-scripts、沒有 allow-same-origin，碰不到宿主 DOM，
// 所以在 iframe 內偽造一個誘餌輸入框，把卡片戳 window.parent/window.top 的動作攔下來轉成 postMessage。
function buildHostBridgeShim(seed: CardStorage): string {
  return `<script>
(function () {
  var parentRef = window.parent;
${buildImeGuardSource()}
${buildStorageShimSource(seed)}

  function notifyHost(text) {
    try {
      parentRef.postMessage({ source: "table-tavern-card", kind: "input", text: text }, "*");
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
 * seed＝這桌上次存下的卡片設定，開場回填進沙盒 localStorage。
 */
export function buildShellDocument(shell: string, seed: CardStorage = {}): string {
  const shim = buildHostBridgeShim(seed);
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
