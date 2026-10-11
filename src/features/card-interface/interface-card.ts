// ST 角色卡的「顯示用 regex 腳本」轉換層：把模型輸出套上卡片自帶的 regex，
// 抽出內嵌的整頁 HTML 介面，再組成可直接餵給沙盒 iframe srcdoc 的文件。
import { buildChatShimSource, scriptLiteral, type CardChat } from "./card-chat-shim";
import { buildMvuShimSource, type CardMvu } from "./mvu/card-mvu-shim";
import { buildSandboxLibs } from "./card-sandbox-libs";
import { CARD_STORAGE_LIMIT, type CardStorage } from "./card-storage";

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
  /** 卡的酒館助手腳本載入 MVU；舊後端沒這欄當 false */
  mvu?: boolean;
}

/** 卡在這桌的穩定代號：角色卡用 character_id，世界書卡（沒有 id）固定 `world`（也是 MVU character 層的身分） */
export function cardKey(card: CardInterface): string {
  return card.character_id || "world";
}

/** 這張卡畫得出東西：不是 DRM／雲端載入器卡，而且有顯示腳本 */
export function cardDrawable(card: CardInterface): boolean {
  return card.unsupported === null && card.scripts.length > 0;
}

/**
 * 一段候選文字與它的來源。腳本按卡隔離（ST 的卡片 regex 只套在那張卡的訊息上）：
 * - `owner`：已知所屬的卡（卡片開場白、已貼出且原文對得上的開場白、骨架合成樓），只給那張卡（cardKey）；
 * - `speaker`：角色台詞樓的說話者，他有自己的介面就只給他的卡；
 * - 其餘（GM 旁白、system 樓、說話者沒介面）：世界書卡先、其餘照桌上順序。
 * 純字串＝沒有來源，照最後一種。`noCache`：這段文字每次選路都不同（前情提要佔位帶亂數），不進快取。
 */
export type ShellCandidate =
  | string
  | null
  | undefined
  | { text: string | null | undefined; speaker?: string; owner?: string; noCache?: boolean };

/** 世界書卡先、其餘照桌上順序 */
export function gmOrder(cards: CardInterface[]): CardInterface[] {
  return [...cards.filter((card) => card.character_id === ""), ...cards.filter((card) => card.character_id !== "")];
}

/** 一段候選文字依來源該試哪幾張卡（依序，先命中先用） */
export function candidateCards(cards: CardInterface[], candidate: ShellCandidate): CardInterface[] {
  const usable = cards.filter(cardDrawable);
  if (typeof candidate === "object" && candidate !== null) {
    if (candidate.owner !== undefined) return usable.filter((card) => cardKey(card) === candidate.owner);
    const speaker = candidate.speaker;
    if (speaker) {
      const own = usable.filter((card) => card.character_id !== "" && card.character_id === speaker);
      if (own.length > 0) return own;
    }
  }
  return gmOrder(usable);
}

/** 「某張卡套某段文字抽得出哪份殼」的快取（呼叫端持有；卡清單一換就換新的一份）。chars＝鍵與殼的總字元數 */
export interface ShellCache {
  entries: Map<string, string | null>;
  chars: number;
}
// 殼動輒幾十 KB、鍵含整樓文字：照總字元數設上限，超過就整份清掉重來（約 40 MB 記憶體）
const SHELL_CACHE_CHARS = 20_000_000;

export function createShellCache(): ShellCache {
  return { entries: new Map(), chars: 0 };
}

/** 單張卡套一段文字：卡內多支腳本照順序接力，抽不到回 null */
export function cardShell(card: CardInterface, text: string, cache?: ShellCache): string | null {
  const key = cache === undefined ? "" : `${cardKey(card)}\u0000${text}`;
  if (cache?.entries.has(key)) return cache.entries.get(key) ?? null;
  const shell = extractShell(applyScripts(text, card.scripts));
  if (cache !== undefined) {
    const size = key.length + (shell?.length ?? 0);
    // 單筆就超過上限的不存：上限是嚴格的
    if (size > SHELL_CACHE_CHARS) return shell;
    if (cache.chars + size > SHELL_CACHE_CHARS) {
      cache.entries.clear();
      cache.chars = 0;
    }
    cache.entries.set(key, shell);
    cache.chars += size;
  }
  return shell;
}

/**
 * 從幾段候選文字裡挑出第一個畫得出來的殼（依序試，先命中先用），每段只試它來源該試的卡、一張一張試，
 * 跨卡不接力。面板與匯入流程共用同一套判斷，才不會出現「按鈕說有、打開卻空的」。
 * index＝命中的是第幾段候選（呼叫端自己對回它代表哪一樓，這不是樓號）；card＝產生這份殼的卡。
 * only＝指定卡（cardKey）：只有那張卡在候選規則下輪得到時才試。
 */
export function findShell(
  cards: CardInterface[],
  candidates: ShellCandidate[],
  options: { only?: string | null; cache?: ShellCache } = {},
): { shell: string; index: number; card: CardInterface } | null {
  for (let index = 0; index < candidates.length; index++) {
    const candidate = candidates[index];
    const described = typeof candidate === "object" && candidate !== null ? candidate : null;
    const text = typeof candidate === "object" && candidate !== null ? candidate.text : candidate;
    if (!text) continue;
    const cache = described?.noCache ? undefined : options.cache;
    for (const card of candidateCards(cards, candidate)) {
      if (options.only != null && cardKey(card) !== options.only) continue;
      const shell = cardShell(card, text, cache);
      if (shell !== null) return { shell, index, card };
    }
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

// 圍欄標記可以是任何字（html、text、html5、x-html…，後面可帶空白）：html 直接收，其餘看內容。
const FENCE_REGEX = /```([^\r\n`]*)\r?\n([\s\S]*?)```/g;
// 頁面起點，圍欄與裸頁面共用；`<htmlfoo>` 這類前綴不算
const PAGE_START = String.raw`<!DOCTYPE\s+html\b|<html(?=[\s>])`;
const SHELL_START_MARKERS = new RegExp(String.raw`^\s*(?:${PAGE_START}|<body(?=[\s>]))`, "i");
const BARE_SHELL_MARKER = new RegExp(PAGE_START, "i");

/**
 * 抽出卡片內嵌的整頁 HTML；抽不到回 null。
 * 圍欄標記為 html，或任何標記但內容以頁面起點開頭，才算殼（```text 包整頁的卡也收，
 * 收尾的 ``` 不會被切進頁面）。沒有圍欄被收時才找裸頁面，而且只看圍欄以外的文字：
 * 被拒收的圍欄（例如 ```json 字串裡含 <!DOCTYPE html>）不會被切出半個頁面。
 */
export function extractShell(rendered: string): string | null {
  const parts: string[] = [];
  let outside = "";
  let last = 0;
  FENCE_REGEX.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = FENCE_REGEX.exec(rendered)) !== null) {
    const lang = match[1].trim().split(/\s+/)[0].toLowerCase();
    const content = match[2];
    if (lang === "html" || SHELL_START_MARKERS.test(content)) {
      parts.push(content);
    }
    outside += rendered.slice(last, match.index);
    last = match.index + match[0].length;
  }
  outside += rendered.slice(last);

  if (parts.length > 0) {
    const joined = parts.join("\n").trim();
    return joined.length > 0 ? joined : null;
  }

  const bare = BARE_SHELL_MARKER.exec(outside);
  if (bare) {
    const tail = outside.slice(bare.index).trim();
    return tail.length > 0 ? tail : null;
  }

  return null;
}

export { CARD_STORAGE_LIMIT, sanitizeCardStorage, type CardStorage } from "./card-storage";

/**
 * Storage 墊片原始碼（純 JS，供 shim 內嵌）：沙盒 iframe 沒有 allow-same-origin，origin 是
 * opaque，碰 localStorage／sessionStorage 一律拋 SecurityError。卡片殼常在初始化就讀設定
 * （Vue setup 裡一句 getItem 就讓整支 app 掛不起來，配上 `[v-cloak] { display: none }` 就是整片白），
 * 所以拿不到就換成同介面的記憶體版；拿得到的環境原封不動。
 *
 * localStorage 那份額外接宿主：開場用 seed 回填上次的值，之後每次寫入把整份快照 postMessage
 * 回去存，殼重掛才留得住玩家調過的設定。sessionStorage 照其語意只活在這次掛載，不外送。
 */
/** 送給宿主的訊息帶上文件 token（宿主一律核 token 才收）；沒給 token 就不帶 */
function tokenField(token: string | undefined): string {
  return token === undefined ? "" : `, token: ${scriptLiteral(token)}`;
}

export function buildStorageShimSource(seed: CardStorage = {}, token?: string): string {
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
        // 上限照桌檔契約量 UTF-8 位元組
        if (new TextEncoder().encode(payload).length > ${CARD_STORAGE_LIMIT}) {
          console.warn("[table-tavern] 卡片存的設定超過上限，這次不存回宿主");
          return;
        }
        hostRef.postMessage({ source: "table-tavern-card", kind: "storage", entries: JSON.parse(payload)${tokenField(token)} }, "*");
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

/**
 * Esc 回報墊片原始碼（純 JS，供 shim 內嵌）：宿主那條 Esc 監聽掛在自己的 window 上，玩家一點
 * 介面裡的東西焦點就進了這支沙盒 iframe，keydown 發生在另一個 document、不跨 document 冒泡，
 * 宿主再也收不到——介面本來就是拿來點的，等於 Esc 必死。改由 iframe 這側回報給宿主。
 *
 * capture 階段且不碰 preventDefault／stopPropagation：Esc 一定關得掉覆蓋層（第三方卡的 handler
 * 吃不掉它），卡片自己的 handler 照樣收到。內嵌位置必須在 IME 守衛之後——注音選字按 Esc 是取消
 * 組字，守衛的 stopImmediatePropagation 會先把它吃掉，面板才不會誤關。
 */
export function buildEscapeShimSource(token?: string): string {
  return `
  window.addEventListener(
    "keydown",
    function (event) {
      if (event.key !== "Escape") return;
      try {
        parentRef.postMessage({ source: "table-tavern-card", kind: "close"${tokenField(token)} }, "*");
      } catch (error) {
        console.warn("[table-tavern] 無法通知宿主關閉卡片介面", error);
      }
    },
    true
  );
`;
}

/**
 * 破圖收斂原始碼（純 JS，供 shim 內嵌）：外部圖片抓不到時（圖床倒站、被擋、沒網路）不顯示瀏覽器的破圖示與
 * alt 字，版面位置照留。抓不到的 <img> 標上 data-tt-broken、用注入的樣式藏起來；之後同一張圖載入成功
 * （卡片 onerror 換了有效網址）就移除標記。只動自己的標記，卡片自己設的 visibility／class 不碰。
 * 在 document 的 capture 階段聽：資源的 error／load 不冒泡，而元素的 load 事件依 DOM 規格不傳到 window，
 * 掛 window 收不到恢復那一下。
 */
export function buildBrokenImageSource(): string {
  return `
  (function () {
    var style = document.createElement("style");
    style.textContent = "img[data-tt-broken]{visibility:hidden!important}";
    (document.head || document.documentElement).appendChild(style);
    function isImage(target) {
      return target && target.tagName === "IMG";
    }
    document.addEventListener(
      "error",
      function (event) {
        if (isImage(event.target)) event.target.setAttribute("data-tt-broken", "");
      },
      true
    );
    document.addEventListener(
      "load",
      function (event) {
        if (isImage(event.target)) event.target.removeAttribute("data-tt-broken");
      },
      true
    );
  })();
`;
}

// 宿主橋接墊片：沙盒 iframe 是 allow-scripts、沒有 allow-same-origin，碰不到宿主 DOM，
// 所以在 iframe 內偽造一個誘餌輸入框，把卡片戳 window.parent/window.top 的動作攔下來轉成 postMessage。
function buildHostBridgeShim(seed: CardStorage, token?: string): string {
  return `<script>
(function () {
  var parentRef = window.parent;
${buildImeGuardSource()}
${buildEscapeShimSource(token)}
${buildBrokenImageSource()}
${buildStorageShimSource(seed, token)}

  function notifyHost(text) {
    try {
      parentRef.postMessage({ source: "table-tavern-card", kind: "input", text: text${tokenField(token)} }, "*");
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
 * chat＝本場讀訊息快照與這份文件的 token；mvu＝MVU 變數快照（null＝這桌沒有 MVU 卡，不定義 MVU 函式）。
 * 順序：內建全域庫 → 讀訊息 → MVU → 橋接 → 卡片自己的 script。讀訊息與 MVU 墊片趕在橋接墊片覆寫
 * window.parent 之前收好真 parent。chat 為 null＝讀訊息與 MVU 函式都不定義，卡片照它自己的退路走。
 */
export function buildShellDocument(
  shell: string,
  seed: CardStorage = {},
  chat: { chat: CardChat; token: string; mvu?: CardMvu | null } | null = null,
): string {
  const chatShim = chat === null ? "" : `<script>${buildChatShimSource(chat.chat, chat.token)}</script>`;
  const mvuShim =
    chat === null || !chat.mvu ? "" : `<script>${buildMvuShimSource(chat.mvu, chat.token)}</script>`;
  const shim = buildSandboxLibs() + chatShim + mvuShim + buildHostBridgeShim(seed, chat?.token);
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
