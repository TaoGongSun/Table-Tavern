// 對拍案例的輸入（預期值由 gen-world-info-fixtures.mjs 用網頁版實作跑出）。改這裡＝重跑產生腳本、兩邊同一筆 commit。
import type { JsonObject } from "../src/features/cards/card-file";
import type { EntryCase, RegexCase, ScanCase, SortCase } from "../src/features/sillytavern/world-info-parity-runner";
import { MVU_WI_SETTINGS, ST_WI_SETTINGS, type WiSettings } from "../src/features/sillytavern/world-info-scan";

const ST = ST_WI_SETTINGS;
const MVU = MVU_WI_SETTINGS;
const with_ = (over: Partial<WiSettings>): WiSettings => ({ ...ST, ...over });

/** 物件形條目，沒給的欄位照 fromWorldFile 預設。 */
const e = (id: string, raw: JsonObject) => ({ id, raw: { comment: id, content: id, ...raw } });

/** 新到舊 */
const CHAT = ["Seraphine: Close the door, the snow is coming in.", "Alice: I push the door open and the cold wind rushes in.", "Seraphine: The hearth at the lantern inn is still burning."];
const CJK_CHAT = ["瑟拉: 先把門關上，雪灌進來了。", "Alice: 我推開門，那條龍睡著。"];

const base = (name: string, entries: ScanCase["entries"], over: Partial<ScanCase> = {}): ScanCase => ({
  name,
  entries,
  chat: CHAT,
  settings: ST,
  maxContext: null,
  ...over,
});

export const scanCases: ScanCase[] = [
  // 主鍵、大小寫、全字
  base("primary key, case-insensitive", [e("a", { key: ["DOOR"] }), e("b", { key: ["dragon"] })]),
  base("whole words: inside a word does not match", [e("a", { key: ["now"] }), e("b", { key: ["snow"] })]),
  base("whole words off for one entry", [e("a", { key: ["now"], matchWholeWords: false })]),
  base("multi-word key falls back to substring", [e("a", { key: ["the cold"] }), e("b", { key: ["cold wi"] })]),
  base("case-sensitive entry", [e("a", { key: ["door"], caseSensitive: true }), e("b", { key: ["Door"], caseSensitive: true })]),
  base("case-sensitive setting", [e("a", { key: ["Close"] }), e("b", { key: ["close"] })], { settings: with_({ caseSensitive: true }) }),
  base("CJK whole word", [e("a", { key: ["龍"] }), e("b", { key: ["推開"] }), e("c", { key: ["雪灌"] })], { chat: CJK_CHAT }),
  base("regex keys", [e("a", { key: ["/d(oo|ra)r/i"] }), e("b", { key: ["/^Seraphine/"] }), e("c", { key: ["/a(/"] }), e("d", { key: ["/\\bwind\\b/"] })]),
  base("plain key that looks like a broken regex", [e("a", { key: ["/a(/"] })], { chat: ["Bob: write /a(/ here"] }),
  base("keys are substituted then trimmed", [e("a", { key: ["  {{user}} "] }), e("b", { key: ["{{char}}"] })], { chat: ["Alice: Bob, wake up."] }),
  base("whitespace-only key matches anything", [e("a", { key: ["   "] })]),
  base("first matching key wins (later keys not substituted)", [e("a", { key: ["zzz", "door", "{{user}}"] })]),
  base("no keys, null keys, disabled", [e("a", { key: [] }), e("b", { key: null }), e("c", { key: ["door"], disable: true })]),

  // 次要鍵
  base("secondary AND_ANY", [e("a", { key: ["door"], keysecondary: ["bandit", "snow"], selectiveLogic: 0 }), e("b", { key: ["door"], keysecondary: ["bandit"], selectiveLogic: 0 })]),
  base("secondary NOT_ALL", [e("a", { key: ["door"], keysecondary: ["bandit", "snow"], selectiveLogic: 1 }), e("b", { key: ["door"], keysecondary: ["cold", "snow"], selectiveLogic: 1 })]),
  base("secondary NOT_ANY", [e("a", { key: ["door"], keysecondary: ["bandit", "snow"], selectiveLogic: 2 }), e("b", { key: ["door"], keysecondary: ["bandit"], selectiveLogic: 2 })]),
  base("secondary AND_ALL", [e("a", { key: ["door"], keysecondary: ["cold", "snow"], selectiveLogic: 3 }), e("b", { key: ["door"], keysecondary: ["cold", "bandit"], selectiveLogic: 3 })]),
  base("secondary ignored when not selective", [e("a", { key: ["door"], keysecondary: ["bandit"], selective: false, selectiveLogic: 3 })]),
  base("secondary keys substituted", [e("a", { key: ["door"], keysecondary: ["{{user}}"], selectiveLogic: 3 })]),

  // 掃描範圍
  base("scan depth 2 and entry override", [e("a", { key: ["hearth"] }), e("b", { key: ["hearth"], scanDepth: 3 }), e("c", { key: ["door"], scanDepth: 0 })]),
  base("fractional scan depth", [e("a", { key: ["door"], scanDepth: 0.5 }), e("b", { key: ["wind"], scanDepth: 2.9 })]),
  base("global scan fields", [e("a", { key: ["watcher"], matchCharacterDescription: true }), e("b", { key: ["watcher"] }), e("c", { key: ["tea"], matchPersonaDescription: true }), e("d", { key: ["ruins"], matchScenario: true })], {
    globalScan: { characterDescription: "The night watcher Seraphine.", personaDescription: "Alice loves tea.", scenario: "" },
  }),
  base("empty messages keep their slot", [e("a", { key: ["door"] }), e("b", { key: ["hearth"] })], { chat: ["", "Seraphine: Close the door.", "Seraphine: The hearth."] }),

  // constant、裝飾、觸發類型
  base("constant and decorators", [
    e("a", { constant: true }),
    e("b", { key: ["zzz"], content: "@@activate\nforced" }),
    e("c", { key: ["door"], content: "@@dont_activate\nnever" }),
    e("d", { constant: true, content: "@@@activate\nescaped" }),
  ]),
  base("generation type triggers", [e("a", { key: ["door"], triggers: ["continue"] }), e("b", { key: ["door"], triggers: ["normal", "swipe"] })]),

  // 機率
  base("probability boundary is inclusive", [e("a", { constant: true, probability: 50, order: 2 }), e("b", { constant: true, probability: 30, order: 1 })], { random: [0.5, 0.31] }),
  base("an active sticky entry does not roll", [e("a", { key: ["zzz"], sticky: 3, probability: 10 })], { timed: { sticky: { a: { start: 1, end: 9, protected: false } }, cooldown: {} } }),
  base("probability rolls in order", [e("a", { constant: true, probability: 50, order: 10 }), e("b", { constant: true, probability: 50, order: 5 }), e("c", { constant: true, probability: 100, order: 1 }), e("d", { constant: true, probability: 10, useProbability: false, order: 0 })], {
    random: [0.4, 0.7],
  }),

  // 遞迴
  base("recursion activates level by level", [e("a", { key: ["door"], content: "the lantern" }), e("b", { key: ["lantern"], content: "an old key" }), e("c", { key: ["key"], content: "end" })]),
  base("prevent and exclude recursion", [e("a", { key: ["door"], content: "lantern", preventRecursion: true }), e("b", { key: ["door"], content: "bell" }), e("c", { key: ["lantern"] }), e("d", { key: ["bell"], excludeRecursion: true }), e("e", { key: ["bell"] })]),
  base("delay until recursion levels", [e("a", { key: ["door"], content: "lantern" }), e("b", { key: ["door"], delayUntilRecursion: true, content: "bell" }), e("c", { key: ["door"], delayUntilRecursion: 2 }), e("d", { key: ["bell"] })]),
  base("recursion off", [e("a", { key: ["door"], content: "lantern" }), e("b", { key: ["lantern"] })], { settings: with_({ recursive: false }) }),

  // 預算（code point 計數，含星平面字元）
  base("budget overflow stops later entries and recursion", [
    e("a", { constant: true, order: 30, content: "0123456789" }),
    e("b", { constant: true, order: 20, content: "lantern 0123456789" }),
    e("c", { constant: true, order: 10, content: "short" }),
    e("d", { key: ["lantern"] }),
  ], { maxContext: 100 }),
  base("ignoreBudget still goes in after overflow", [
    e("a", { constant: true, order: 30, content: "01234567890123456789012345" }),
    e("b", { constant: true, order: 20, content: "x" }),
    e("c", { constant: true, order: 10, content: "always", ignoreBudget: true }),
  ], { maxContext: 100 }),
  base("budget counts code points (astral)", [e("a", { constant: true, order: 2, content: "😀😀😀😀😀😀😀😀😀😀" }), e("b", { constant: true, order: 1, content: "abcdefghijk" })], { maxContext: 100 }),
  base("budget cap", [e("a", { constant: true, order: 2, content: "aaaaaaaaaa" }), e("b", { constant: true, order: 1, content: "bbbbbbbbbb" })], { maxContext: 1000, settings: with_({ budgetCap: 15 }) }),
  // 代換後「Alice\n」6 個字放得下預算 8，原文「{{user}}\n」9 個字放不下
  base("content substituted before budget check", [e("a", { constant: true, content: "{{user}}" })], { maxContext: 32 }),
  base("sticky entries get the budget first", [e("a", { key: ["door"], order: 100, content: "aaaaaaaa" }), e("b", { key: ["zzz"], order: 50, sticky: 3, content: "bbbbbbbb" })], {
    maxContext: 40,
    timed: { sticky: { b: { start: 1, end: 9, protected: false } }, cooldown: {} },
  }),
  base("recursion rounds count earlier text against the budget", [
    e("a", { key: ["door"], content: "lantern xxxxxxxxxxx" }),
    e("c", { key: ["lantern"], order: 200, content: "z" }),
    e("b", { key: ["lantern"], order: 50, content: "yyyy" }),
  ], { maxContext: 100 }),

  // 計時
  base("sticky keeps firing without keys; cooldown blocks", [e("a", { key: ["zzz"], sticky: 3 }), e("b", { key: ["door"], cooldown: 2 }), e("c", { key: ["door"], sticky: 2 })], {
    timed: { sticky: { a: { start: 2, end: 5, protected: false } }, cooldown: { b: { start: 1, end: 4, protected: false } } },
  }),
  base("sticky expires into a protected cooldown", [e("a", { key: ["zzz"], sticky: 1, cooldown: 2 })], { timed: { sticky: { a: { start: 1, end: 3, protected: false } }, cooldown: {} } }),
  base("rollback drops unprotected effects", [e("a", { key: ["zzz"], sticky: 5 }), e("b", { key: ["door"], cooldown: 5 })], {
    timed: { sticky: { a: { start: 3, end: 8, protected: false } }, cooldown: { b: { start: 3, end: 8, protected: true } } },
  }),
  base("effects for entries without timers or gone", [e("a", { key: ["door"] }), e("b", { key: ["door"], sticky: 0 })], {
    timed: { sticky: { a: { start: 1, end: 9, protected: false }, gone: { start: 1, end: 2, protected: false } }, cooldown: { b: { start: 1, end: 9, protected: false } } },
  }),
  base("delay holds until enough messages", [e("a", { key: ["door"], delay: 3 }), e("b", { key: ["door"], delay: 4 })]),
  base("zero timers are no timers", [e("a", { key: ["door"], sticky: 0, cooldown: 0, delay: 0 })]),

  // 群組
  base("group override wins by order", [e("a", { key: ["door"], group: "g", groupOverride: true, order: 5 }), e("b", { key: ["door"], group: "g", groupOverride: true, order: 9 }), e("c", { key: ["door"], group: "g" })]),
  base("group weighted roll", [e("a", { key: ["door"], group: "g", groupWeight: 10 }), e("b", { key: ["door"], group: "g", groupWeight: 30 }), e("c", { key: ["door"], group: "g", groupWeight: 60 })], { random: [0.5] }),
  base("group weight boundary is inclusive", [e("a", { key: ["door"], group: "g", groupWeight: 50 }), e("b", { key: ["door"], group: "g", groupWeight: 50 })], { random: [0.5] }),
  base("group scoring keeps the best match", [e("a", { key: ["door", "snow"], group: "g", useGroupScoring: true }), e("b", { key: ["door"], group: "g", useGroupScoring: true }), e("c", { key: ["door", "snow", "wind"], group: "g" })], {
    random: [0.1],
  }),
  base("group scoring from settings", [e("a", { key: ["door", "snow"], group: "g" }), e("b", { key: ["door"], group: "g" })], { settings: with_({ useGroupScoring: true }), random: [0.9] }),
  base("sticky member keeps its group", [e("a", { key: ["door"], group: "g", sticky: 3 }), e("b", { key: ["door"], group: "g" })], { timed: { sticky: { a: { start: 1, end: 9, protected: false } }, cooldown: {} } }),
  base("entry in two groups removed twice (D31)", [e("a", { key: ["door"], group: "g1, g2", groupWeight: 1 }), e("b", { key: ["door"], group: "g1", groupWeight: 100 }), e("c", { key: ["door"], group: "g2", groupWeight: 100 }), e("d", { key: ["door"] })], {
    random: [0.99, 0.99],
  }),
  base("numeric group names iterate first", [e("a", { key: ["door"], group: "zz" }), e("b", { key: ["door"], group: "zz" }), e("c", { key: ["door"], group: "10" }), e("d", { key: ["door"], group: "10" }), e("e", { key: ["door"], group: "2" }), e("f", { key: ["door"], group: "2" })], {
    random: [0.1, 0.9, 0.6],
  }),
  base("group already activated blocks recursion members", [e("a", { key: ["door"], group: "g", content: "lantern" }), e("b", { key: ["lantern"], group: "g" })]),

  // 插入位置、順序
  base("positions and order", [
    e("b1", { constant: true, position: 0, order: 1 }),
    e("b2", { constant: true, position: 0, order: 2 }),
    e("a1", { constant: true, position: 1, order: 1 }),
    e("emt", { constant: true, position: 5 }),
    e("emb", { constant: true, position: 6 }),
    e("ant", { constant: true, position: 2 }),
    e("anb", { constant: true, position: 3 }),
    e("d4s", { constant: true, position: 4, depth: 4, role: 0 }),
    e("d4u", { constant: true, position: 4, depth: 4, role: 1 }),
    e("d2a", { constant: true, position: 4, depth: 2, role: 2, order: 7 }),
    e("d2a2", { constant: true, position: 4, depth: 2, role: 2, order: 3 }),
    e("out1", { constant: true, position: 7, outletName: "hud" }),
    e("out2", { constant: true, position: 7, outletName: "hud", order: 50 }),
    e("outx", { constant: true, position: 7, outletName: "" }),
    e("weird", { constant: true, position: 9 }),
    e("empty", { constant: true, content: "" }),
  ]),
  base("same order keeps load order", [e("x", { constant: true, order: 5 }), e("y", { constant: true, order: 5 }), e("z", { constant: true, order: 5 })]),
  base("order coercion", [e("s", { constant: true, order: "7" }), e("n", { constant: true, order: 3 }), e("t", { constant: true, order: true }), e("u", { constant: true, order: null })]),

  // MVU 設定
  base("MVU settings: no whole words, no names, full budget", [e("a", { key: ["now"] }), e("b", { key: ["oor"] })], { settings: MVU }),
];

export const regexCases: RegexCase[] = [
  { name: "plain", key: "/dragon/", haystack: "\u0001a dragon" },
  { name: "i flag", key: "/DRAGON/i", haystack: "\u0001a dragon" },
  { name: "g flag ignored", key: "/dragon/g", haystack: "\u0001a dragon" },
  { name: "y anchors at the start (scan text starts with \\x01)", key: "/wolf/y", haystack: "\u0001wolf" },
  { name: "y matches when the pattern covers the start", key: "/\\x01wolf/y", haystack: "\u0001wolf" },
  { name: "duplicate flags reject", key: "/a/ii", haystack: "a" },
  { name: "unknown flag is not a regex", key: "/a/x", haystack: "/a/x" },
  { name: "unescaped slash is not a regex", key: "/a/b/", haystack: "a/b" },
  { name: "escaped slash", key: "/a\\/b/", haystack: "xa/b" },
  { name: "no m: ^ is start only", key: "/^door/", haystack: "\u0001x\ndoor" },
  { name: "m: ^ after newline", key: "/^door/m", haystack: "\u0001x\ndoor" },
  { name: "m: ^ after carriage return", key: "/^door/m", haystack: "\u0001x\rdoor" },
  { name: "m: $ before U+2028", key: "/door$/m", haystack: "\u0001door x" },
  { name: "no s: . skips \\n", key: "/a.b/", haystack: "a\nb" },
  { name: "no s: . skips U+2028", key: "/a.b/", haystack: "a b" },
  { name: "no s: . skips \\r", key: "/a.b/", haystack: "a\rb" },
  { name: "s: . takes \\n", key: "/a.b/s", haystack: "a\nb" },
  { name: "\\d is ASCII only (fullwidth digit)", key: "/\\d/", haystack: "３" },
  { name: "\\d is ASCII only with u", key: "/\\d/u", haystack: "３" },
  { name: "\\w is ASCII only", key: "/\\w/", haystack: "龍" },
  { name: "\\w is ASCII only with u", key: "/\\w/u", haystack: "é" },
  { name: "\\W matches CJK", key: "/\\W龍/", haystack: "那龍" },
  { name: "\\s takes U+3000", key: "/a\\sb/", haystack: "a　b" },
  { name: "\\s takes U+FEFF", key: "/a\\sb/", haystack: "a﻿b" },
  { name: "\\s does not take U+0085", key: "/a\\sb/", haystack: "a\u0085b" },
  { name: "\\b is an ASCII boundary", key: "/\\b龍/", haystack: "那龍" },
  { name: "\\b between ASCII words", key: "/\\bcat\\b/", haystack: "a cat." },
  { name: "\\B inside a word", key: "/c\\Bat/", haystack: "cat" },
  { name: "identity escape without u", key: "/\\「門\\」/", haystack: "「門」" },
  { name: "identity escape with u is an error", key: "/\\「/u", haystack: "「" },
  { name: "escaped dash outside a class", key: "/a\\-b/", haystack: "a-b" },
  { name: "escaped caret is literal", key: "/\\^a/", haystack: "^a" },
  { name: "lone brace is literal without u", key: "/a{b/", haystack: "a{b" },
  { name: "lone brace with u is an error", key: "/a{b/u", haystack: "a{b" },
  { name: "lone closing bracket", key: "/a]/", haystack: "a]" },
  { name: "quantifier braces", key: "/a{2,3}/", haystack: "caab" },
  { name: "class with \\d", key: "/[\\d]/", haystack: "x7" },
  { name: "class with dot is literal", key: "/[.]/", haystack: "ab" },
  { name: "class with \\W", key: "/[\\W]/", haystack: "abc" },
  { name: "class with \\W matches space", key: "/[\\W]/", haystack: "a b" },
  { name: "negated class with \\W", key: "/[^\\W]/", haystack: "!!a" },
  { name: "class with \\s and \\d", key: "/[\\s\\d]/", haystack: "a　" },
  { name: "class range", key: "/[a-c]x/", haystack: "bx" },
  { name: "class dash after class escape is literal", key: "/[\\w-!]/", haystack: "-" },
  { name: "class caret not first is literal", key: "/[a^]/", haystack: "^" },
  { name: "class \\b is backspace", key: "/[\\b]/", haystack: "\b" },
  { name: "empty class never matches", key: "/a[]/", haystack: "a" },
  { name: "negated empty class matches anything", key: "/a[^]b/", haystack: "a\nb" },
  { name: "hex and unicode escapes", key: "/\\x41\\u0042/", haystack: "AB" },
  { name: "u code point escape", key: "/\\u{1F600}/u", haystack: "😀" },
  { name: "surrogate pair escape", key: "/\\uD83D\\uDE00/", haystack: "😀" },
  { name: "control escape", key: "/\\cJ/", haystack: "\n" },
  { name: "backreference", key: "/(a)\\1/", haystack: "aa" },
  { name: "legacy octal without groups", key: "/\\101/", haystack: "A" },
  { name: "\\8 is a literal 8", key: "/\\8/", haystack: "8" },
  { name: "named group and backreference", key: "/(?<w>o)\\k<w>/", haystack: "door" },
  { name: "\\k without named groups is literal k", key: "/\\k/", haystack: "k" },
  { name: "lookbehind", key: "/(?<=推)開/", haystack: "推開" },
  { name: "negative lookahead", key: "/door(?!s)/", haystack: "doors door" },
  { name: "lazy quantifier", key: "/a+?b/", haystack: "aab" },
  { name: "alternation", key: "/cat|dog/", haystack: "hotdog" },
  { name: "unicode property with u", key: "/\\p{L}/u", haystack: "龍" },
  { name: "\\p without u is literal p", key: "/\\p/", haystack: "p" },
  { name: "invalid group is not a regex", key: "/a(/", haystack: "a(" },
  {
    name: "without u, . takes half of an astral char (known difference)",
    key: "/^..$/",
    haystack: "😀",
    knownDifference: { parsed: true, matches: false, note: "JS 不帶 u 以 UTF-16 碼元比對，桌面版以 code point 比對" },
  },
  { name: "with u, . takes the whole astral char", key: "/^.$/u", haystack: "😀" },
  { name: "i flag with \\w does not fold long s", key: "/\\w/i", haystack: "ſ" },
  { name: "\\0 is NUL", key: "/a\\0b/", haystack: "a\u0000b" },
  {
    name: "lone surrogate escape (JS accepts, Rust cannot represent)",
    key: "/\\uD83D/",
    haystack: "😀",
    knownDifference: { parsed: false, matches: null, note: "落單的代理 Rust 字串表示不了，當一般字串" },
  },
  { name: "no m: $ is the end only", key: "/door$/", haystack: "door\nx" },
  { name: "lone closing bracket with u is an error", key: "/a]/u", haystack: "a]" },
  { name: "\\D outside a class", key: "/a\\Db/", haystack: "axb" },
  { name: "\\S outside a class", key: "/a\\Sb/", haystack: "a b" },
  { name: "mixed class with \\W", key: "/[a\\W]/", haystack: "!" },
  { name: "mixed class with \\W, word char outside", key: "/x[a\\W]/", haystack: "xb" },
  // 不合法的量詞序列：JS SyntaxError → 一般字串
  { name: "quantifier after quantifier", key: "/a*+/", haystack: "aaa" },
  { name: "double plus", key: "/a++b/", haystack: "aab" },
  { name: "plus after braces", key: "/a{1,2}+/", haystack: "aa" },
  { name: "quantified word boundary", key: "/\\b*/", haystack: "a" },
  { name: "quantified caret", key: "/^*a/", haystack: "a" },
  { name: "quantified lookbehind", key: "/(?<=a)*b/", haystack: "ab" },
  { name: "nothing to repeat", key: "/*a/", haystack: "a" },
  { name: "lazy quantifier is fine", key: "/a*?b/", haystack: "aab" },
  { name: "quantified lookahead is fine without u", key: "/(?=a)*a/", haystack: "a" },
  { name: "lazy then another quantifier", key: "/a*??/", haystack: "a" },
  { name: "lookahead with min 1 keeps its capture", key: "/(?=(a))+\\1/", haystack: "a" },
  { name: "lookahead with min 0 drops its capture", key: "/^(?=(a))*\\1b/", haystack: "ab" },
  { name: "optional lookahead drops its capture", key: "/^(?=(a))?\\1b/", haystack: "ab" },
  { name: "lookahead with {0,2} drops its capture", key: "/^(?=(a)){0,2}\\1b/", haystack: "ab" },
  { name: "lookahead with min 0 still matches without the capture", key: "/^(?=(a))*\\1a/", haystack: "ab" },
  // fancy-regex 在重複群組的下一輪不清掉上一輪的捕捉，JS 會清
  {
    name: "repeated group resets its own capture each round",
    key: "/^(a\\1)+$/",
    haystack: "aa",
    knownDifference: { parsed: true, matches: false, note: "fancy-regex 重複群組不重設上一輪的捕捉" },
  },
  {
    name: "repeated group resets captures of a branch not taken",
    key: "/^(?:(a)|b)+\\1$/",
    haystack: "ab",
    knownDifference: { parsed: true, matches: false, note: "fancy-regex 重複群組不重設上一輪的捕捉" },
  },
  { name: "optional lookahead that cannot match", key: "/(?=x)?a/", haystack: "a" },
  { name: "lookahead with braces", key: "/(?=a){2}a/", haystack: "a" },
  { name: "lookahead with braces out of order", key: "/(?=a){2,1}a/", haystack: "ac" },
  { name: "negative lookahead with braces out of order", key: "/(?!b){3,2}a/", haystack: "ac" },
  { name: "braces out of order", key: "/a{2,1}/", haystack: "aa" },
  { name: "lookahead quantifier then lazy", key: "/(?=a)*?a/", haystack: "a" },
  // 沒參與比對、或在後面的群組：反向參照當空字串
  { name: "backreference to a group in another branch", key: "/(a)|\\1b/", haystack: "b" },
  { name: "forward backreference", key: "/\\1(a)/", haystack: "a" },
  { name: "backreference to an optional group that did not match", key: "/(a)?\\1b/", haystack: "b" },
  { name: "backreference to a nested group that did not match", key: "/(a|(b))\\2c/", haystack: "ac" },
  { name: "named backreference to a group that did not match", key: "/(?<x>a)?\\k<x>b/", haystack: "b" },
  // 類別內的 \c（Annex B）
  { name: "class control with a digit", key: "/[\\c1]/", haystack: "\u0011" },
  { name: "class control with underscore", key: "/[\\c_]/", haystack: "\u001f" },
  { name: "class control with a letter", key: "/[\\cJ]/", haystack: "\n" },
  { name: "class \\c with other is a backslash", key: "/[\\c*]/", haystack: "\\" },
  // i 旗標與屬性名的已知差異
  {
    name: "class \\w with i folds long s in Rust",
    key: "/[\\w]/i",
    haystack: "ſ",
    knownDifference: { parsed: true, matches: true, note: "類別內的 \\w 遇到 i，Rust 的大小寫折疊會吃到 ſ" },
  },
  {
    name: "class range with i folds Kelvin sign in Rust",
    key: "/[a-z]/i",
    haystack: "\u212a",
    knownDifference: { parsed: true, matches: true, note: "Rust 的大小寫折疊把 K 當成 k" },
  },
  {
    name: "short script property name",
    key: "/\\p{Han}/u",
    haystack: "龍",
    knownDifference: { parsed: true, matches: true, note: "JS 要寫 Script=Han，Rust 收簡寫" },
  },
];

export const entryCases: EntryCase[] = [
  { name: "world file: defaults", form: "worldFile", raw: { content: "x" } },
  {
    name: "world file: every field",
    form: "worldFile",
    raw: {
      key: ["a", 1, "b"],
      keysecondary: ["c"],
      comment: "t",
      content: "body",
      constant: true,
      selective: false,
      order: 7,
      position: 4,
      excludeRecursion: true,
      preventRecursion: true,
      delayUntilRecursion: 2,
      disable: true,
      probability: 30,
      useProbability: false,
      depth: 2,
      selectiveLogic: 3,
      outletName: "hud",
      group: "g",
      groupOverride: true,
      groupWeight: 7,
      scanDepth: 3,
      caseSensitive: true,
      matchWholeWords: false,
      useGroupScoring: true,
      role: 2,
      sticky: 3,
      cooldown: 4,
      delay: 5,
      matchPersonaDescription: true,
      matchCharacterDescription: true,
      matchCharacterPersonality: true,
      matchCharacterDepthPrompt: true,
      matchScenario: true,
      matchCreatorNotes: true,
      triggers: ["normal", 3],
      ignoreBudget: true,
    },
  },
  { name: "world file: key not an array", form: "worldFile", raw: { key: "a", content: "x" } },
  { name: "world file: null order and wrong types", form: "worldFile", raw: { order: null, probability: "50", sticky: "2", delayUntilRecursion: "1", caseSensitive: 1, content: "x" } },
  { name: "world file: decorators", form: "worldFile", raw: { content: "@@activate\n@@unknown\n@@@dont_activate\nbody\n@@activate" } },
  { name: "world file: only decorators keeps content", form: "worldFile", raw: { content: "@@activate\n@@dont_activate" } },
  { name: "world file: escaped decorator after unknown", form: "worldFile", raw: { content: "@@unknown\n@@@activate\nbody" } },
  { name: "character book: defaults (no enabled means disabled)", form: "characterBook", raw: { keys: ["a"], content: "x" } },
  {
    name: "character book: extensions",
    form: "characterBook",
    raw: {
      keys: ["a"],
      secondary_keys: ["b"],
      comment: "t",
      content: "x",
      constant: true,
      selective: true,
      insertion_order: 12,
      enabled: true,
      position: "after_char",
      extensions: {
        position: 4,
        exclude_recursion: true,
        prevent_recursion: true,
        delay_until_recursion: true,
        probability: 40,
        useProbability: false,
        depth: 1,
        selectiveLogic: 2,
        outlet_name: "o",
        group: "g",
        group_override: true,
        group_weight: 3,
        scan_depth: 5,
        case_sensitive: false,
        match_whole_words: true,
        use_group_scoring: false,
        role: 1,
        sticky: 1,
        cooldown: 2,
        delay: 3,
        match_persona_description: true,
        match_character_description: true,
        match_character_personality: true,
        match_character_depth_prompt: true,
        match_scenario: true,
        match_creator_notes: true,
        triggers: ["swipe"],
        ignore_budget: true,
      },
    },
  },
  { name: "character book: position string only", form: "characterBook", raw: { keys: ["a"], content: "x", enabled: true, position: "before_char" } },
  { name: "character book: enabled truthiness", form: "characterBook", raw: { keys: ["a"], content: "x", enabled: 1 } },
  { name: "character book: keys not an array", form: "characterBook", raw: { keys: null, content: "x", enabled: true } },
];

/** 決定性的偽亂數（LCG），產生長陣列的排序案例。 */
function lcg(seed: number) {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
}

function mixed(length: number, seed: number, odd: number): SortCase["items"] {
  const next = lcg(seed);
  return Array.from({ length }, () => {
    const roll = next();
    if (roll < odd / 3) return {};
    if (roll < (odd * 2) / 3) return { order: "x" };
    if (roll < odd) return { order: String(Math.floor(next() * 10)) };
    return { order: Math.floor(next() * 8) };
  });
}

export const sortCases: SortCase[] = [
  { name: "number, non-numeric string, number, number", items: [{ order: 1 }, { order: "x" }, { order: 3 }, { order: 1 }] },
  { name: "undefined between numbers", items: [{ order: 0 }, {}, { order: 30 }, { order: 30 }] },
  { name: "coercible values", items: [{ order: null }, { order: true }, { order: "5" }, { order: " 7 " }, { order: [] }, { order: [2] }, { order: {} }, { order: 3 }] },
  { name: "all numbers, many ties, length 300", items: Array.from({ length: 300 }, (_, index) => ({ order: (index * 37) % 11 })) },
  { name: "mixed, length 70", items: mixed(70, 1, 0.3) },
  { name: "mixed, length 100", items: mixed(100, 7, 0.2) },
  { name: "mixed, length 257", items: mixed(257, 42, 0.1) },
  { name: "mostly sorted with a few non-numbers, length 400", items: Array.from({ length: 400 }, (_, index) => (index % 53 === 0 ? {} : { order: 1000 - index })) },
  { name: "short mixed", items: mixed(20, 3, 0.4) },
  { name: "length 5 with NaN", items: [{ order: 775 }, { order: 41 }, { order: 0 }, { order: "x" }, { order: 3 }] },
  { name: "length 6 with undefined", items: [{ order: 2 }, {}, { order: 9 }, { order: 1 }, { order: "y" }, { order: 5 }] },
  { name: "length 7 mixed", items: mixed(7, 11, 0.5) },
  { name: "length 7 descending with NaN", items: [{ order: 1 }, { order: 2 }, { order: "z" }, { order: 3 }, { order: 4 }, {}, { order: 5 }] },
];
