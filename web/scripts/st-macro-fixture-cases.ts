// 巨集對拍案例的輸入（預期值由 gen-st-macro-fixtures.mjs 用網頁版實作跑出）。改這裡＝重跑產生腳本、兩邊同一筆 commit。
// 補 st-macro-cases.json（ST 官方案例）沒涵蓋的：卡欄位、歷史、時間（固定時鐘）、{{pick}}（固定 chatId）、變數型別與鍵順序。
import { FIXTURE_NOW_MS, type MacroCase } from "../src/features/sillytavern/macro-parity-runner";

const c = (name: string, input: string, over: Omit<MacroCase, "name" | "input"> = {}): MacroCase => ({ name, input, ...over });
const HOUR = 3_600_000;
const DAY = 24 * HOUR;

export const macroCases: MacroCase[] = [
  // ── 名字、舊式標記、卡欄位
  c("names and legacy markers", "{{user}}／{{char}}／<USER>／<bot>／<Char>／<GROUP>／<CHARIFNOTGROUP>／{{group}}／{{groupNotMuted}}／{{notChar}}"),
  c("card fields get one macro pass without card fields", "{{description}}|{{personality}}|{{scenario}}|{{charInstruction}}|{{version}}|{{persona}}|{{charPrompt}}"),
  c("card fields off", "[{{description}}][{{scenario}}][{{greeting}}]", { options: { replaceCharacterCard: false } }),
  c("greetings by index", "{{greeting}}|{{greeting::1}}|{{greeting::2}}|{{charFirstMessage::0}}|{{greeting::-1}}|{{greeting::x}}"),
  c("mes examples", "{{mesExamples}}||{{mesExamplesRaw}}", { context: { card: { mes_example: "{{user}}: 嗨\n<start>\n{{char}}: 你好\n<START>" } } }),
  c(
    "lazy card fields run their side effects once, on first use",
    "{{getvar::n}}|{{description}}|{{description}}|{{getvar::n}}|{{greeting::1}}{{getvar::g}}",
    { context: { card: { description: "{{incvar::n}}號", alternate_greetings: ["{{setvar::g::一}}A", "{{setvar::g::二}}B"] } } },
  ),
  c("card field trims and drops CR", "[{{scenario}}]", { context: { card: { scenario: "  雨\r\n夜 \r" } } }),
  c("case-insensitive names, spaces inside braces, unknown macros", "{{USER}} {{ Char }} {{unknownMacro::{{user}}}} {{user::extra}}"),
  c("model, input, limits, generation type, mobile", "{{model}}|{{input}}|{{maxPrompt}}|{{maxContext}}|{{maxResponseTokens}}|{{lastGenerationType}}|{{isMobile}}|{{hasExtension::x}}", {
    context: { model: "gpt-x", input: "草稿", limits: { maxContext: 8192, maxResponse: 1024.5 }, generationType: "normal" },
  }),
  c("original is one-shot", "{{original}}+{{original}}", { options: { original: "原文" } }),
  c("original without one stays raw", "{{original}}"),
  c("outlets", "[{{outlet::門}}][{{outlet::沒有}}][{{outlet::}}]", { context: { outlets: { 門: "門後的內容" } } }),

  // ── 對話
  c("chat history", "{{lastMessage}}|{{lastUserMessage}}|{{lastCharMessage}}|{{lastMessageId}}|{{firstIncludedMessageId}}|{{firstDisplayedMessageId}}|{{allChatRange}}|{{lastSwipeId}}"),
  c("empty chat", "[{{lastMessage}}][{{lastMessageId}}][{{allChatRange}}][{{firstIncludedMessageId}}][{{idle_duration}}]", { context: { chat: [] } }),
  c("idle duration skips the latest message", "{{idle_duration}}|{{idleDuration}}", {
    context: {
      chat: [
        { isUser: true, text: "舊", sentAt: FIXTURE_NOW_MS - 3 * DAY },
        { isUser: true, text: "前", sentAt: FIXTURE_NOW_MS - 2 * HOUR },
        { isUser: true, text: "最新", sentAt: FIXTURE_NOW_MS - 1000 },
      ],
    },
  }),
  c("idle duration without time", "{{idle_duration}}", { context: { chat: [{ isUser: true, text: "a" }, { isUser: false, text: "b" }] } }),

  // ── 時間（固定時鐘、Asia/Taipei）
  c("time macros", "{{time}}|{{time::UTC+8}}|{{time_UTC-2}}|{{time_utc+5}}|{{time::UTC+20}}|{{time::UTC-0}}|{{date}}|{{weekday}}|{{isotime}}|{{isodate}}"),
  c("datetimeformat tokens", "{{datetimeformat::YYYY-MM-DD HH:mm:ss.SSS Z ZZ [Q]Q X x S SS SSSS SSSSSSSSS}}"),
  c("datetimeformat week and era tokens", "{{datetimeformat::DDD DDDD DDDo Do E e d do dd ddd dddd w ww wo W WW Wo gg gggg ggggg GG GGGG GGGGG N NN NNN NNNN NNNNN y yy yyy yyyy yo}}"),
  c("datetimeformat hours and meridiem", "{{datetimeformat::h hh H HH k kk a A hmm hmmss Hmm Hmmss m mm s ss z zz Y YY YYYYY YYYYYY M Mo MM MMM MMMM Q}}"),
  c("datetimeformat long formats and escapes", "{{datetimeformat::LT|LTS|L|LL|LLL|LLLL|l|ll|lll|llll|[LT [Q]]|\\Y\\Q|w|x|[unclosed}}"),
  c("datetimeformat empty format uses the default", "{{datetimeformat::}}"),
  c("time around new year", "{{datetimeformat::YYYY-MM-DD W GGGG w gggg ddd}}", { context: { nowMs: Date.UTC(2027, 0, 1, 4) } }),
  c("time early morning (24h clock)", "{{datetimeformat::k kk h a}}", { context: { nowMs: Date.UTC(2026, 9, 7, 16, 30) } }),
  c("timeDiff ISO forms", [
    "{{timeDiff::2026-01-01::2026-01-03}}",
    "{{timeDiff::2026-10-07T15:05:09Z::2026-10-07T10:00:00+05:00}}",
    "{{timeDiff::2026-W41-3::2026-280}}",
    "{{timeDiff::20261007::2026-10}}",
    "{{timeDiff::2026-10-07 23:59:59.999::2026-10-07T24:00}}",
    "{{timeDiff::2026::2025-12-31T23:00}}",
    "{{timeDiff::+002026-10-07::2026-10-07T00:00:45}}",
    "{{timeDiff::2026-10-07T12:30:15,5+0800::2026-10-07T12:30}}",
  ].join("|")),
  c("timeDiff invalid and other formats", [
    "{{timeDiff::2026-02-30::2026-01-01}}",
    "{{timeDiff::2026-10-07T25:00::2026-01-01}}",
    "{{timeDiff::2026-W54::2026-01-01}}",
    "{{timeDiff::2026-13-01::2026-01-01}}",
    "{{timeDiff::Wed, 07 Oct 2026 15:05:09 GMT::Tue, 06 Oct 2026 15:05:09 +0000}}",
    "{{timeDiff::/Date(1791385509123)/::2026-10-07T15:00:00Z}}",
  ].join("|")),
  c("timeDiff RFC 2822 overflow", [
    "{{timeDiff::Sat, 32 Jan 2026 10:00:00 GMT::01 Jan 2026 00:00 GMT}}",
    "{{timeDiff::31 Feb 2026 10:00 GMT::01 Jan 2026 00:00 GMT}}",
    "{{timeDiff::29 Feb 2026 10:00 GMT::01 Jan 2026 00:00 GMT}}",
    "{{timeDiff::29 Feb 2028 10:00 GMT::01 Jan 2026 00:00 GMT}}",
    "{{timeDiff::31 Apr 2026 10:00 GMT::01 Jan 2026 00:00 GMT}}",
    "{{timeDiff::07 Oct 2026 25:02 GMT::01 Jan 2026 00:00 GMT}}",
    "{{timeDiff::07 Oct 2026 23:61 GMT::01 Jan 2026 00:00 GMT}}",
    "{{timeDiff::07 Oct 2026 24:00:00 GMT::01 Jan 2026 00:00 GMT}}",
    "{{timeDiff::07 Oct 2026 10:00:60 GMT::01 Jan 2026 00:00 GMT}}",
  ].join("|")),
  c("humanize thresholds", [
    "{{timeDiff::2026-01-01T00:00:44Z::2026-01-01T00:00:00Z}}",
    "{{timeDiff::2026-01-01T00:00:45Z::2026-01-01T00:00:00Z}}",
    "{{timeDiff::2026-01-01T00:01:30Z::2026-01-01T00:00:00Z}}",
    "{{timeDiff::2026-01-01T00:44:00Z::2026-01-01T00:00:00Z}}",
    "{{timeDiff::2026-01-01T00:45:00Z::2026-01-01T00:00:00Z}}",
    "{{timeDiff::2026-01-01T21:00:00Z::2026-01-01T00:00:00Z}}",
    "{{timeDiff::2026-01-02T12:00:00Z::2026-01-01T00:00:00Z}}",
    "{{timeDiff::2026-01-26::2026-01-01}}",
    "{{timeDiff::2026-03-01::2026-01-01}}",
    "{{timeDiff::2026-12-01::2026-01-01}}",
    "{{timeDiff::2029-01-01::2026-01-01}}",
  ].join("|")),

  // ── 亂數、pick、擲骰
  c("random takes the sequence", "{{random::甲::乙::丙}}|{{random 甲, 乙\\, 丙}}|{{random:甲::乙}}|{{random}}|{{random::只有}}", { context: { random: [0.99, 0.5, 0.4, 0.7] } }),
  c("roll formulas", "{{roll 6}}|{{roll::2d6+1}}|{{roll::d20-3}}|{{roll::3D4}}|{{roll::nope}}|{{roll::0d6}}|{{roll::1001d2}}", { context: { random: [0.5, 0.1, 0.9, 0.3, 0.2, 0.8, 0.6] } }),
  c("pick is seeded by chat id, content and position", "{{pick::甲::乙::丙::丁::戊::己::庚::辛}}{{pick::甲::乙::丙::丁::戊::己::庚::辛}}", { context: { chatId: "table-42" } }),
  c("pick position counts UTF-16 units", "😀莫拉{{pick::a::b::c::d::e::f::g::h::i::j}}😀{{pick a,b,c,d,e,f,g,h,i,j}}", { context: { chatId: "web-save-1" } }),
  c("pick inside an argument uses its absolute position", "前綴{{reverse::{{pick::一::二::三::四::五}}}}", { context: { chatId: "chat-9" } }),

  // ── 語法
  c("comments, escapes, trim, noop", "a{{// 註解 {{setvar::c::1}} }}b|\\{\\{user\\}\\}|上一行\n\n{{trim}}\n下一行|x{{noop}}y|{{{user}}}|{{user|{{getvar::c}}"),
  c("scoped if/else with nesting, inversion and shorthand", "{{if user}}有{{else}}沒有{{/if}}|{{if !user}}有{{else}}沒有{{/if}}|{{if 0}}A{{/if}}|{{if off}}B{{else}}C{{/if}}|{{if 1}}外{{if 0}}內{{else}}內否{{/if}}{{/if}}|{{if::1::行內}}|{{if .hp}}有血{{else}}沒血{{/if}}|{{if $g}}G{{/if}}", {
    context: { variables: { local: { hp: "3" }, global: { g: "0" } } },
  }),
  c("scoped content dedents unless #", "{{if 1}}\n    縮排\n      兩行\n{{/if}}|{{#if 1}}\n  保留  \n{{/if}}|[{{trim}}  內容  {{/trim}}]|{{/if}}多餘"),
  c("scoped content with unknown and list macros", "{{foo}}x{{/foo}}|{{random}}a{{/random}}|{{reverse}}abc{{/reverse}}"),
  c("nested arguments evaluate inside-out", "{{setvar::k::v}}{{reverse::{{getvar::k}}{{newline::2}}{{space}}}}|{{getvar::{{reverse::k}}}}"),
  c("argument types and arity", "{{newline::abc}}|a{{newline::2}}b{{space::3}}c|{{space::-1}}|{{reverse}}|{{setvar::only}}|{{getvar::a::b}}"),
  c("deep nesting", `${"{{reverse::".repeat(20)}深${"}}".repeat(20)}`),
  c("legacy colon and whitespace arguments", "{{reverse:a::b}}|{{reverse a b}}|{{roll 1d1}}|{{reverse :x}}", { context: { random: [0] } }),

  // ── 變數：型別轉換、鍵順序、索引
  c("variables keep JSON types from the store", "{{getvar::n}}|{{getvar::s}}|{{getvar::b}}|{{getvar::z}}|{{getvar::arr}}|{{getvar::obj}}|{{getvar::one}}|{{getvar::blank}}|{{hasvar::z}}|{{hasvar::missing}}", {
    context: { variables: { local: { n: 1.5, s: " 007 ", b: true, z: null, arr: [1, "x"], obj: { k: [1] }, one: [3], blank: "  " } } },
  }),
  c("addvar and incvar across types", "{{addvar::n::2}}{{incvar::n}}|{{addvar::s::人}}|{{addvar::list::x}}{{getvar::list}}|{{incvar::obj}}|{{decvar::fresh}}|{{addvar::arr::1}}|{{addvar::f::0.1}}{{addvar::f::0.2}}{{getvar::f}}|{{addvar::big::1e21}}{{getvar::big}}", {
    context: { variables: { local: { n: "1", s: "旅", list: "[1]", obj: { a: 1 }, arr: [1, 2] } } },
  }),
  c("indexed variables", "{{setvarkey::m::甲::1}}{{setvarkey::m::10::x}}{{setvarkey::m::2::y}}{{getvar::m}}|{{setvarkey::a::2::z}}{{getvar::a}}|{{getvarkey::a::2}}|{{getvarkey::a::length}}|{{getvarkey::s::1}}|{{getvarkey::s::length}}|{{setvarkey::n::0::q}}{{getvar::n}}|{{getvarkey::o::k}}|{{getvarkey::o::missing}}", {
    context: { variables: { local: { s: "\"abc\"", n: "5", o: "{\"k\":{\"z\":1}}" } } },
  }),
  c("indexed variables on arrays with odd keys", "{{setvarkey::a::1.5::x}}{{getvar::a}}|{{setvarkey::a::-0::y}}{{getvar::a}}|{{setvarkey::a::length::4}}{{getvar::a}}|{{setvarkey::a::foo::z}}{{getvar::a}}|{{getglobalvarkey::g::0}}", {
    context: { variables: { local: { a: "[ 1 , 2 ]" }, global: { g: "[\"甲\"]" } } },
  }),
  c("delete, global variables and aliases", "{{setglobalvar::g::1}}{{getglobalvar::g}}|{{getvar::g}}|{{globalvarexists::g}}|{{flushglobalvar::g}}{{hasglobalvar::g}}|{{varexists::x}}|{{setvarindex::x::0::a}}{{getvarindex::x::0}}|{{deletevar::x}}{{getvar::x}}", {
    context: { variables: { local: { x: "1" }, global: { keep: 1 } } },
  }),
  c("key order follows JS objects", "{{setvar::b::1}}{{setvar::2::x}}{{setvar::a::2}}{{setvar::1::y}}{{setvar::b::3}}", { context: { variables: { local: { z: 0, "10": 1 } } } }),
  c("variable shorthand operators", "{{.hp=10}}{{.hp}}|{{.hp++}}|{{.hp--}}|{{.hp-=3}}{{.hp}}|{{.hp+=2}}{{.hp}}|{{.hp>5}}|{{.hp>=9}}|{{.hp<9}}|{{.hp<=9}}|{{.hp==9}}|{{.hp!=9}}|{{.s+=x}}{{.s}}|{{.hp-=abc}}{{.hp}}"),
  c("variable shorthand fallbacks", "{{.missing??預設}}|{{.missing||後備}}|{{.zero||後備}}|{{.off||後備}}|{{.zero??有}}|{{$g??=7}}|{{$g}}|{{$g??=8}}|{{.e||=填}}|{{.e}}|{{.w = {{user}} }}{{.w}}|{{.cmp>abc}}", {
    context: { variables: { local: { zero: "0", off: "off", e: "", cmp: "1" } } },
  }),
  c("empty variable name throws and stays raw", "{{setvar::::1}}|{{addvar::::1}}|{{incvar::}}|{{deletevar::}}x"),
];
