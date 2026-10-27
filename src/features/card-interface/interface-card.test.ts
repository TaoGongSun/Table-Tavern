import { describe, expect, it } from "vitest";
import {
  applyScripts,
  candidateCards,
  cardCarriesPage,
  cardShell,
  cardDrawable,
  buildEscapeShimSource,
  buildImeGuardSource,
  buildShellDocument,
  buildStorageShimSource,
  CARD_STORAGE_LIMIT,
  extractShell,
  createShellCache,
  findShell,
  parseStRegex,
  sanitizeCardStorage,
  type CardInterface,
  type CardStorage,
  type InterfaceScript,
} from "./interface-card";

const NAMES = { user: "玩家", char: "卡" };

function makeScript(overrides: Partial<InterfaceScript>): InterfaceScript {
  return {
    name: "test",
    find_regex: "",
    replace_string: "",
    trim_strings: [],
    min_depth: null,
    max_depth: null,
    ...overrides,
  };
}

describe("parseStRegex", () => {
  it("解析 /樣式/旗標 形式並保留 dotAll", () => {
    const regex = parseStRegex("/foo.bar/s");
    expect(regex).not.toBeNull();
    expect(regex?.flags).toBe("s");
    expect(regex?.test("foo\nbar")).toBe(true);
  });

  it("裸樣式（無包夾斜線）可直接編譯使用", () => {
    const regex = parseStRegex("\\s*请选择你的身份\\s*");
    expect(regex).not.toBeNull();
    expect(regex?.test("  请选择你的身份  ")).toBe(true);
  });

  it("丟掉 JS 不認得的旗標，其餘旗標照常生效", () => {
    const regex = parseStRegex("/abc/gx");
    expect(regex).not.toBeNull();
    expect(regex?.flags).toBe("g");
    expect(regex?.test("abc")).toBe(true);
  });

  it("壞樣式回 null，不丟例外", () => {
    expect(parseStRegex("/[/")).toBeNull();
  });
});

describe("applyScripts", () => {
  it("套用西幻卡真實 find_regex：$1/$2 換入、$& 與 $( 不被 JS 語意吃掉", () => {
    const script = makeScript({
      find_regex:
        "/<GoldenRPG_UI>.*?<CurrentView>([\\s\\S]*?)<\\/CurrentView>.*?<WorldSystem>([\\s\\S]*?)<\\/WorldSystem>.*?<\\/GoldenRPG_UI>/s",
      replace_string: "```html\n<!DOCTYPE html><body><div>$1</div><div>$2</div><span>$&與$(jQuery)</span></body>\n```",
    });
    const raw = "<GoldenRPG_UI><CurrentView>正文</CurrentView><WorldSystem>世界</WorldSystem></GoldenRPG_UI>";

    const result = applyScripts(raw, [script], NAMES);

    expect(result).toContain("<div>正文</div>");
    expect(result).toContain("<div>世界</div>");
    expect(result).toContain("$&與$(jQuery)");
  });

  it("{{match}} 換成整段命中文字；trim_strings 先從命中文字挖掉指定片段", () => {
    const script = makeScript({
      find_regex: "\\[(.*?)\\]",
      replace_string: "before {{match}} after, group=$1",
      trim_strings: ["DROPME"],
    });
    const raw = "trim me: [KEEP-DROPME]";

    const result = applyScripts(raw, [script], NAMES);

    expect(result).toBe("trim me: before [KEEP-] after, group=KEEP-");
    expect(result).not.toContain("DROPME");
  });

  it("中間一支腳本樣式壞掉時只跳過該支，前後兩支照常套用", () => {
    const scripts = makeThreeScriptsWithBadMiddle();

    const result = applyScripts("AAA", scripts, NAMES);

    expect(result).toBe("CCC");
  });
});

describe("applyScripts（抓取引用與 {{user}}／{{char}}，照 ST runRegexScript）", () => {
  const run = (raw: string, find: string, replace: string, trim: string[] = [], names = NAMES) =>
    applyScripts(raw, [makeScript({ find_regex: find, replace_string: replace, trim_strings: trim })], names);

  it("$N 任意位數、$0、$<名稱>、{{MATCH}}；越界、未命中、不存在的群組換空字串", () => {
    expect(run("abcdefghij", "/(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)/", "[$10|$1|$01]")).toBe("[j|a|a]");
    expect(run("ab", "/(?<x>a)b/", "[$0|$<x>|{{MATCH}}|{{Match}}]")).toBe("[ab|a|ab|ab]");
    expect(run("ab", "/(a)(z)?b/", "[$2|$3|$99|$<nope>]")).toBe("[|||]");
  });

  it("抓取值只代換一輪：命中文字裡的 $1、{{match}} 原樣；$&、$( 維持字面", () => {
    expect(run("<UI>$1 與 {{match}}</UI>", "/<UI>(.*?)<\\/UI>/", "<p>$1</p>$&$(x)")).toBe("<p>$1 與 {{match}}</p>$&$(x)");
  });

  it("替換字串與抓取文字裡的 {{user}}／{{char}} 都換（不分大小寫、內側可有空白）", () => {
    expect(run("<UI>歡迎 {{user}}</UI>", "/<UI>(.*?)<\\/UI>/", "{{User}} 對 {{ char }}：$1")).toBe("玩家 對 卡：歡迎 玩家");
  });

  it("名字含 {{char}}、$1 不會再被代換；名字空白就保留字面", () => {
    expect(run("<UI>z</UI>", "/<UI>(.)<\\/UI>/", "{{user}}:$1:{{char}}", [], { user: "{{char}}$1", char: "$<x>" })).toBe(
      "{{char}}$1:z:$<x>",
    );
    expect(run("<UI>", "/<UI>/", "{{user}}|{{char}}", [], { user: " ", char: "" })).toBe("{{user}}|{{char}}");
  });

  it("trim_strings 先代換名字再挖，整段命中、位置與具名群組都挖", () => {
    expect(run("<UI>卡：嗨</UI>", "/<UI>(?<t>.*?)<\\/UI>/", "$1|$<t>|{{match}}", ["{{char}}："])).toBe("嗨|嗨|<UI>嗨</UI>");
  });

  it("其他巨集與卡片的 Vue 模板原樣保留", () => {
    expect(run("<UI>", "/<UI>/", "{{ attributes.str }}{{ text }}{{random::a::b}}{{getvar::x}}")).toBe(
      "{{ attributes.str }}{{ text }}{{random::a::b}}{{getvar::x}}",
    );
  });
});

function makeThreeScriptsWithBadMiddle(): InterfaceScript[] {
  return [
    makeScript({ find_regex: "/A/g", replace_string: "B" }),
    makeScript({ find_regex: "/[/", replace_string: "永遠用不到" }),
    makeScript({ find_regex: "/B/g", replace_string: "C" }),
  ];
}

describe("extractShell", () => {
  it("抽出 ```html 圍欄內的內容", () => {
    const rendered = "前言文字\n```html\n<!DOCTYPE html><html><body>介面</body></html>\n```\n後記";

    expect(extractShell(rendered)).toBe("<!DOCTYPE html><html><body>介面</body></html>");
  });

  it("沒有圍欄但找得到 <!DOCTYPE html> 時，從該處取到字串結尾", () => {
    const rendered = "模型先講了一段話。<!DOCTYPE html><html><body>介面</body></html>";

    expect(extractShell(rendered)).toBe("<!DOCTYPE html><html><body>介面</body></html>");
  });

  it("純文字找不到殼時回 null", () => {
    expect(extractShell("這裡只有純文字，沒有任何 HTML 介面")).toBeNull();
  });

  const page = "<!DOCTYPE html><html><body>介面</body></html>";

  it.each([
    ["text", "```text\n"],
    ["html5", "```html5\n"],
    ["x-html", "```x-html\n"],
    ["標記後帶空白", "```text  \n"],
    ["大寫標記", "```TEXT\n"],
    ["CRLF", "```text\r\n"],
    ["空白標記", "```\n"],
  ])("任意標記（%s）包整頁：收內容、不帶收尾 ```", (_label, open) => {
    const shell = extractShell(`前言\n${open}${page}\n\`\`\`\n後記`);
    expect(shell).toBe(page);
  });

  it("html 標記大小寫不分，非頁面片段也收", () => {
    expect(extractShell("```HTML\n<div>片段</div>\n```")).toBe("<div>片段</div>");
  });

  it("小寫 doctype 與 <html lang> 也算頁面起點", () => {
    expect(extractShell("```text\n<!doctype html><p>a</p>\n```")).toBe("<!doctype html><p>a</p>");
    expect(extractShell('```text\n<html lang="zh"><p>a</p></html>\n```')).toBe(
      '<html lang="zh"><p>a</p></html>',
    );
  });

  it("多個圍欄：html 與 text 包的頁面都收，json 不收", () => {
    const rendered = [
      "```json\n{\"a\": 1}\n```",
      "```html\n<style>x</style>\n```",
      "```text\n<body>主體</body>\n```",
    ].join("\n");
    // 各段照原樣接起來（圍欄內容自帶結尾換行）
    expect(extractShell(rendered)).toBe("<style>x</style>\n\n<body>主體</body>");
  });

  it("```json 字串裡含 <!DOCTYPE html>：圍欄不收，也不走裸頁面切出半個頁面", () => {
    const rendered = '```json\n{"html": "<!DOCTYPE html><html><body>x</body></html>"}\n```\n後記';
    expect(extractShell(rendered)).toBeNull();
  });

  it("被拒收的圍欄以外仍有裸頁面：從圍欄外的起點切", () => {
    const rendered = '```json\n{"a": "<html>"}\n```\n說明。<!DOCTYPE html><html><body>介面</body></html>';
    expect(extractShell(rendered)).toBe("<!DOCTYPE html><html><body>介面</body></html>");
  });

  it("<htmlfoo> 這類前綴不算頁面起點", () => {
    expect(extractShell("```text\n<htmlfoo>不是頁面</htmlfoo>\n```")).toBeNull();
    expect(extractShell("<htmlfoo>不是頁面</htmlfoo>")).toBeNull();
    expect(extractShell("```text\n<bodyguard>不是頁面</bodyguard>\n```")).toBeNull();
  });
});

describe("buildShellDocument", () => {
  it("輸出含誘餌 textarea、__ttHost 橋接物件與 triggerSlash", () => {
    const doc = buildShellDocument("<html><head></head><body>殼內容</body></html>");

    expect(doc).toContain("send_textarea");
    expect(doc).toContain("__ttHost");
    expect(doc).toContain("triggerSlash");
  });

  it("殼裡的 window.parent.document 字面會被改寫成 window.__ttHost.document", () => {
    const shell = "<html><head></head><body><script>window.parent.document.getElementById('x')</script></body></html>";

    const doc = buildShellDocument(shell);

    expect(doc).toContain("window.__ttHost.document.getElementById('x')");
    expect(doc).not.toContain("window.parent.document.getElementById('x')");
  });
});

describe("findShell", () => {
  const card = (over: Partial<CardInterface> = {}): CardInterface => ({
    character_id: "c1",
    character_name: "卡",
    scripts: [
      { name: "殼", find_regex: "/<UI>([\\s\\S]*?)<\\/UI>/s", replace_string: "```html\n<!DOCTYPE html><body>$1</body>\n```", trim_strings: [], min_depth: null, max_depth: null },
    ],
    unsupported: null,
    opening: "<UI>開場</UI>",
    ...over,
  });

  it("依序試候選文字，先命中的先用", () => {
    const match = findShell([card()], ["沒有標籤的旁白", "<UI>最新一則</UI>", "<UI>更舊的</UI>"], { userName: "玩家" });
    expect(match?.shell).toContain("最新一則");
    expect(match?.index).toBe(1);
  });

  it("畫不出來的卡（DRM／雲端載入器）不參與，沒腳本就回 null", () => {
    expect(findShell([card({ unsupported: "scrypt" })], ["<UI>開場</UI>"], { userName: "玩家" })).toBeNull();
    expect(findShell([card({ scripts: [] })], ["<UI>開場</UI>"], { userName: "玩家" })).toBeNull();
  });

  it("候選文字全對不上就回 null，空值直接跳過", () => {
    expect(findShell([card()], [null, undefined, "", "純文字"], { userName: "玩家" })).toBeNull();
  });

  it("按卡隔離：A 卡的輸出不交給 B 卡接著轉；回傳命中的卡", () => {
    const relayA = card({ character_id: "A", scripts: [{ ...card().scripts[0], find_regex: "/<X>/", replace_string: "<UI>接力</UI>" }] });
    const plainB = card({ character_id: "B" });
    expect(findShell([relayA, plainB], ["<X>"], { userName: "玩家" })).toBeNull();
    expect(findShell([relayA, plainB], ["<UI>直接</UI>"], { userName: "玩家" })?.card.character_id).toBe("B");
  });

  it("開場白只給所屬那張卡；說話者有自己的卡就只給他", () => {
    const a = card({ character_id: "A" });
    const b = card({ character_id: "B", scripts: [{ ...card().scripts[0], replace_string: "```html\n<!DOCTYPE html><body>B</body>\n```" }] });
    expect(findShell([a, b], [{ text: "<UI>開場</UI>", owner: "B" }], { userName: "玩家" })?.shell).toContain("<body>B</body>");
    expect(findShell([a, b], [{ text: "<UI>台詞</UI>", speaker: "B" }], { userName: "玩家" })?.card.character_id).toBe("B");
    expect(findShell([a, b], [{ text: "<UI>台詞</UI>", speaker: "沒卡的人" }], { userName: "玩家" })?.card.character_id).toBe("A");
    expect(findShell([a, b], [{ text: "<UI>開場</UI>", owner: "A" }], { userName: "玩家", only: "B" })).toBeNull();
  });

  it("快取：同一張卡同一段文字只套一次腳本；帶 noCache 的候選不進快取", () => {
    const cache = createShellCache();
    const a = card();
    findShell([a], ["<UI>一</UI>"], { userName: "玩家", cache });
    expect(cache.entries.size).toBe(1);
    expect(cache.chars).toBeGreaterThan(0);
    cache.entries.set([...cache.entries.keys()][0], "快取裡的殼");
    expect(findShell([a], ["<UI>一</UI>"], { userName: "玩家", cache })?.shell).toBe("快取裡的殼");
    findShell([a], [{ text: "<UI>佔位</UI>", noCache: true }], { userName: "玩家", cache });
    expect(cache.entries.size).toBe(1);
  });

  it("{{char}} 用命中的那張卡的名字（非 MVU 卡也一樣），{{user}} 用傳進來的玩家名", () => {
    const named = (id: string, name: string) =>
      card({
        character_id: id,
        character_name: name,
        scripts: [{ ...card().scripts[0], replace_string: "```html\n<!DOCTYPE html><body>{{user}}見{{char}}:$1</body>\n```" }],
      });
    const cards = [named("A", "甲"), named("B", "乙")];
    expect(findShell(cards, [{ text: "<UI>x</UI>", speaker: "B" }], { userName: "旅人" })?.shell).toBe(
      "<!DOCTYPE html><body>旅人見乙:x</body>",
    );
    expect(findShell(cards, ["<UI>x</UI>"], { userName: "旅人" })?.shell).toBe("<!DOCTYPE html><body>旅人見甲:x</body>");
  });

  it("快取鍵含玩家名：同一份快取改名後拿到新名字的殼", () => {
    const cache = createShellCache();
    const a = card({ scripts: [{ ...card().scripts[0], replace_string: "```html\n<!DOCTYPE html><body>{{user}}</body>\n```" }] });
    expect(findShell([a], ["<UI>一</UI>"], { userName: "舊名", cache })?.shell).toContain("舊名");
    expect(findShell([a], ["<UI>一</UI>"], { userName: "新名", cache })?.shell).toContain("新名");
    expect(cardShell(a, "<UI>一</UI>", "舊名", cache)).toContain("舊名");
  });

  it("單筆就超過上限的不存", () => {
    const cache = createShellCache();
    const huge = `<UI>${"字".repeat(20_000_001)}</UI>`;
    expect(findShell([card()], [huge], { userName: "玩家", cache })).not.toBeNull();
    expect(cache.entries.size).toBe(0);
    expect(cache.chars).toBe(0);
  });

  it("快取超過總字元上限就整份清掉重來", () => {
    const cache = createShellCache();
    cache.chars = 19_999_990;
    cache.entries.set("舊", null);
    findShell([card()], ["<UI>這段文字加殼超過上限</UI>"], { userName: "玩家", cache });
    expect(cache.entries.has("舊")).toBe(false);
    expect(cache.entries.size).toBe(1);
    expect(cache.chars).toBeLessThan(1000);
  });
});

describe("buildStorageShimSource", () => {
  // 沙盒 iframe 的 window：Storage 一碰就拋，parent 收得到 postMessage
  const sandboxWindow = (sent: unknown[] = []) => ({
    parent: { postMessage: (message: unknown) => sent.push(message) },
    get localStorage(): Storage {
      throw new Error("SecurityError");
    },
    get sessionStorage(): Storage {
      throw new Error("SecurityError");
    },
  });

  const run = (win: Record<string, unknown>, seed?: CardStorage) => {
    new Function("window", "console", buildStorageShimSource(seed))(win, console);
    return win;
  };

  it("Storage 一碰就拋（沙盒 iframe 的 opaque origin）時換成記憶體版", () => {
    const win = run(sandboxWindow());

    const local = win.localStorage as Storage;
    expect(local.getItem("gameSettings")).toBeNull();
    local.setItem("gameSettings", '{"theme":"nord"}');
    expect(local.getItem("gameSettings")).toBe('{"theme":"nord"}');
    expect(local.length).toBe(1);
    expect(local.key(0)).toBe("gameSettings");
    local.removeItem("gameSettings");
    expect(local.length).toBe(0);
    // 兩份各自獨立，卡片拿 sessionStorage 當暫存不會污染 localStorage
    (win.sessionStorage as Storage).setItem("tab", "map");
    expect(local.getItem("tab")).toBeNull();
  });

  it("Storage 本來就能用就原封不動", () => {
    const real = { getItem: () => null } as unknown as Storage;
    const win = run({ localStorage: real, sessionStorage: real });

    expect(win.localStorage).toBe(real);
    expect(win.sessionStorage).toBe(real);
  });

  it("seed 開場就在 localStorage 裡，sessionStorage 不吃 seed", () => {
    const win = run(sandboxWindow(), { gameSettings: '{"theme":"synthwave"}' });

    expect((win.localStorage as Storage).getItem("gameSettings")).toBe('{"theme":"synthwave"}');
    expect((win.sessionStorage as Storage).getItem("gameSettings")).toBeNull();
  });

  it("localStorage 每次寫入把整份快照送回宿主，sessionStorage 不外送", () => {
    const sent: any[] = [];
    const win = run(sandboxWindow(sent), { theme: "nord" });

    (win.localStorage as Storage).setItem("fontSize", "1.2");
    expect(sent).toEqual([
      { source: "table-tavern-card", kind: "storage", entries: { theme: "nord", fontSize: "1.2" } },
    ]);

    (win.localStorage as Storage).removeItem("theme");
    expect(sent[1].entries).toEqual({ fontSize: "1.2" });

    (win.sessionStorage as Storage).setItem("tab", "map");
    expect(sent).toHaveLength(2);
  });

  it("卡片塞爆上限就不送回宿主，沙盒裡照樣讀得到", () => {
    const sent: unknown[] = [];
    const win = run(sandboxWindow(sent));

    (win.localStorage as Storage).setItem("blob", "x".repeat(CARD_STORAGE_LIMIT + 1));

    expect(sent).toEqual([]);
    expect((win.localStorage as Storage).getItem("blob")).toHaveLength(CARD_STORAGE_LIMIT + 1);
  });

  it("值裡的 </script> 不會提前關掉墊片那支 script", () => {
    const doc = buildShellDocument("<html><head></head><body>殼</body></html>", {
      evil: "</script><script>window.pwned=1</script>",
    });

    expect(doc).not.toContain("</script><script>window.pwned=1");
    expect(doc).toContain("\\u003c/script>");
  });
});

describe("buildImeGuardSource", () => {
  // 卡片殼的 handler 綁在 window（bubble）；守衛在 capture 階段先跑，攔下就傳不到卡片
  const install = () => {
    const registered: { type: string; handler: (event: unknown) => void; capture: unknown }[] = [];
    const win = {
      addEventListener: (type: string, handler: (event: unknown) => void, capture: unknown) =>
        registered.push({ type, handler, capture }),
    };
    new Function("window", buildImeGuardSource())(win);
    return registered[0];
  };

  const keydown = (over: Record<string, unknown>) => {
    const calls = { stopped: 0, prevented: 0 };
    const event = {
      key: "Enter",
      isComposing: false,
      keyCode: 13,
      stopImmediatePropagation: () => calls.stopped++,
      preventDefault: () => calls.prevented++,
      ...over,
    };
    return { event, calls };
  };

  it("掛在 window 的 capture 階段，才搶得贏卡片自己的 keydown", () => {
    const guard = install();

    expect(guard.type).toBe("keydown");
    expect(guard.capture).toBe(true);
  });

  it("組字中的按鍵不往下傳，但不擋預設行為（輸入法照樣選字）", () => {
    const guard = install();

    const composing = keydown({ isComposing: true, keyCode: 229 });
    guard.handler(composing.event);
    expect(composing.calls).toEqual({ stopped: 1, prevented: 0 });

    // isComposing 在 WebKit 某些狀態下拿不到，只靠 keyCode 229 也要攔得住
    const legacy = keydown({ isComposing: undefined, keyCode: 229 });
    guard.handler(legacy.event);
    expect(legacy.calls.stopped).toBe(1);
  });

  it("組字結束後的按鍵照常放行，卡片的 Enter 送出還是能用", () => {
    const guard = install();

    const plain = keydown({});
    guard.handler(plain.event);
    expect(plain.calls).toEqual({ stopped: 0, prevented: 0 });

    const escape = keydown({ key: "Escape", keyCode: 27 });
    guard.handler(escape.event);
    expect(escape.calls.stopped).toBe(0);
  });
});

describe("buildEscapeShimSource", () => {
  // 焦點一進沙盒 iframe，keydown 只在 iframe 自己的 document 發生，宿主的 window 監聽收不到
  const install = () => {
    const registered: { type: string; handler: (event: unknown) => void; capture: unknown }[] = [];
    const sent: unknown[] = [];
    const win = {
      addEventListener: (type: string, handler: (event: unknown) => void, capture: unknown) =>
        registered.push({ type, handler, capture }),
    };
    const parentRef = { postMessage: (data: unknown) => sent.push(data) };
    new Function("window", "parentRef", "console", buildEscapeShimSource())(win, parentRef, console);
    return { listener: registered[0], sent };
  };

  const keydown = (key: string) => {
    const calls = { stopped: 0, prevented: 0 };
    const event = {
      key,
      stopPropagation: () => calls.stopped++,
      preventDefault: () => calls.prevented++,
    };
    return { event, calls };
  };

  it("掛在 window 的 capture 階段，第三方卡的 handler 吃不掉 Esc", () => {
    const { listener } = install();

    expect(listener.type).toBe("keydown");
    expect(listener.capture).toBe(true);
  });

  it("Esc 回報宿主關面板，同時不擋卡片自己的 handler", () => {
    const { listener, sent } = install();

    const escape = keydown("Escape");
    listener.handler(escape.event);
    expect(sent).toEqual([{ source: "table-tavern-card", kind: "close" }]);
    expect(escape.calls).toEqual({ stopped: 0, prevented: 0 });
  });

  it("其他按鍵不回報：在介面裡打字不會把面板關掉", () => {
    const { listener, sent } = install();

    listener.handler(keydown("Enter").event);
    listener.handler(keydown("a").event);
    expect(sent).toEqual([]);
  });

  it("內嵌在 IME 守衛之後，注音選字按 Esc 取消組字時不誤關面板", () => {
    const doc = buildShellDocument("<html><head></head><body>殼</body></html>");

    // 守衛靠 stopImmediatePropagation 吃掉組字中的按鍵，前提是它比這支先註冊
    expect(doc.indexOf("event.isComposing")).toBeLessThan(doc.indexOf(`kind: "close"`));
  });
});

describe("sanitizeCardStorage", () => {
  it("只留字串值，物件以外的形狀一律回 null", () => {
    expect(sanitizeCardStorage({ theme: "nord", size: 3, nested: { a: 1 } })).toEqual({ theme: "nord" });
    expect(sanitizeCardStorage(null)).toBeNull();
    expect(sanitizeCardStorage(["theme"])).toBeNull();
    expect(sanitizeCardStorage("theme")).toBeNull();
  });

  it("整份超過上限回 null，宿主當作沒這份快照", () => {
    expect(sanitizeCardStorage({ blob: "x".repeat(CARD_STORAGE_LIMIT) })).toBeNull();
    expect(sanitizeCardStorage({ blob: "x".repeat(100) })).not.toBeNull();
  });

  it("上限照桌檔契約量 UTF-8 位元組：字元數沒超過、位元組超過的也回 null", () => {
    // 兩萬二千個「中」約 66 KB（字元數只有兩萬二）
    expect(sanitizeCardStorage({ blob: "中".repeat(22_000) })).toBeNull();
    expect(sanitizeCardStorage({ blob: "中".repeat(20_000) })).not.toBeNull();
  });
});

describe("讀不出的原卡（unreadable）", () => {
  it("畫不出、不進候選，同桌其他卡照常", () => {
    const script = makeScript({ find_regex: "/x/", replace_string: "<div>x</div>" });
    const broken: CardInterface = {
      character_id: "c1",
      character_name: "壞",
      scripts: [],
      unsupported: "unreadable",
      opening: null,
      mvu: false,
    };
    const healthy: CardInterface = { ...broken, character_id: "c2", unsupported: null, scripts: [script] };
    expect(cardDrawable(broken)).toBe(false);
    expect(candidateCards([broken, healthy], "x")).toEqual([healthy]);
  });
});

describe("cardCarriesPage（匯入提示的判準）", () => {
  const base: CardInterface = {
    character_id: "c1",
    character_name: "卡",
    scripts: [],
    unsupported: null,
    opening: null,
    mvu: false,
  };
  const page = makeScript({ find_regex: "/<UI>/", replace_string: "<!DOCTYPE html><html><body>x</body></html>" });

  it("只有美化 regex：不算", () => {
    const beauty = makeScript({ find_regex: "/a/", replace_string: "<span class=\"x\">a</span>" });
    expect(cardCarriesPage({ ...base, scripts: [beauty] })).toBe(false);
  });

  it("替換範本帶整頁 HTML：算", () => {
    expect(cardCarriesPage({ ...base, scripts: [page] })).toBe(true);
  });

  it("圍欄包住的整頁也算", () => {
    const fenced = makeScript({ replace_string: "```html\n<html><body>x</body></html>\n```" });
    expect(cardCarriesPage({ ...base, scripts: [fenced] })).toBe(true);
  });

  it("被判不支援的卡不算", () => {
    expect(cardCarriesPage({ ...base, scripts: [page], unsupported: "remote_loader" })).toBe(false);
  });

  it("範本有頁面、開場沒命中 find：仍算（已知限制，匯入先預告、鈕要等模型輸出）", () => {
    const card = { ...base, scripts: [page], opening: "沒有標籤的開場白" };
    expect(cardCarriesPage(card)).toBe(true);
    expect(findShell([card], [card.opening], { userName: "玩家" })).toBeNull();
  });
});
