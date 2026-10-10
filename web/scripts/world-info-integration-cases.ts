// 巨集×世界書掃描對拍案例的輸入（預期值由 gen-world-info-fixtures.mjs 用網頁版實作跑出）。改這裡＝重跑產生腳本、
// 兩邊同一筆 commit。被卡欄位巨集引用的卡，第一輪產物不放巨集（見 integration-parity-runner.ts 開頭）。
import type { JsonObject } from "../src/features/cards/card-file";
import type { IntegrationCase } from "../src/features/chat/integration-parity-runner";

const CARD = { name: "瑟拉", description: "{{char}} 是守夜人，認得 {{user}}。" };
const CHAT = [
  { isUser: false, text: "爐火還在燒。" },
  { isUser: true, text: "我推開燈籠旅店的門，看見一條龍。" },
];

const e = (id: string, raw: JsonObject) => ({ id, raw: { comment: `條目${id}`, ...raw } });

const base = (name: string, entries: IntegrationCase["entries"], over: Partial<IntegrationCase> = {}): IntegrationCase => ({
  name,
  card: CARD,
  userName: "Alice",
  chat: CHAT,
  entries,
  ...over,
});

export const integrationCases: IntegrationCase[] = [
  base("setvar in one entry, getvar in a later one", [
    e("1", { constant: true, order: 200, content: "{{setvar::門::開著}}門已{{getvar::門}}" }),
    e("2", { key: ["龍"], order: 100, content: "龍說：門{{getvar::門}}。" }),
  ]),
  base("keys are substituted from variables", [
    e("1", { key: ["{{getvar::k}}"], content: "取鍵命中" }),
    e("2", { key: ["{{getvar::none}}"], content: "空鍵不中" }),
  ], { variables: { local: { k: "龍" } } }),
  base("recursion runs the side effects of each activated entry once", [
    e("1", { constant: true, order: 200, content: "{{incvar::層}}提到燈塔" }),
    e("2", { key: ["燈塔"], order: 100, content: "{{incvar::層}}燈塔亮著，第{{getvar::層}}層" }),
  ]),
  base("author's note and depth injections", [
    e("1", { constant: true, position: 2, content: "上：{{user}}" }),
    e("2", { constant: true, position: 3, content: "下：{{char}}" }),
    e("3", { constant: true, position: 4, depth: 1, role: 0, content: "深一系統{{incvar::n}}" }),
    e("4", { constant: true, position: 4, depth: 1, role: 1, content: "深一玩家" }),
    e("5", { constant: true, position: 4, depth: 3, role: 2, content: "深三助手{{getvar::n}}" }),
  ]),
  base("outlets: the scan reads the previous round, this round replaces it", [
    e("1", { constant: true, order: 300, position: 7, outletName: "線索", content: "新線索{{setvar::o::1}}" }),
    e("2", { constant: true, order: 200, position: 0, content: "前：{{outlet::線索}}" }),
    e("3", { constant: true, order: 100, position: 1, content: "後：{{getvar::o}}" }),
  ], { prevOutlets: { 線索: "舊線索" } }),
  base("card field macros in entries", [e("1", { constant: true, content: "卡說：{{description}}／{{personality}}" })]),
  base("global variables and random picks", [
    e("1", { constant: true, order: 200, content: "{{setglobalvar::g::{{random::甲::乙}}}}{{getglobalvar::g}}" }),
    e("2", { constant: true, order: 100, content: "挑：{{pick::紅::綠::藍}}" }),
  ], { variables: { global: { 舊: 1 } } }),
  base("the second pass expands macro text produced by the first", [e("1", { constant: true, content: "{{getvar::tpl}}" })], {
    variables: { local: { tpl: "{{user}} 在此" } },
  }),
  base("card fields with side effects run once, card field macros reuse the first pass", [
    e("1", { constant: true, order: 200, content: "卡：{{description}}" }),
    e("2", { constant: true, order: 100, position: 1, content: "{{personality}}／n={{getvar::n}}" }),
  ], { card: { name: "瑟拉", description: "{{incvar::n}}號守夜人 {{char}}" } }),
  base("first-pass output with side effects: before, after, the card, then injections from shallow to deep", [
    e("1", { constant: true, position: 0, content: "{{getvar::t1}}" }),
    e("2", { constant: true, position: 1, content: "{{getvar::t2}}" }),
    e("3", { constant: true, position: 4, depth: 1, role: 0, content: "{{getvar::t3}}深一" }),
    e("4", { constant: true, position: 4, depth: 3, role: 0, content: "{{getvar::t4}}深三" }),
    e("5", { constant: true, position: 2, content: "{{getvar::t5}}註記" }),
    e("6", { constant: true, position: 4, depth: 1, role: 1, content: "{{getvar::t6}}深一玩家" }),
  ], {
    card: { name: "瑟拉", description: "{{char}} 守夜。{{getvar::t0}}" },
    variables: {
      local: {
        t0: "{{addvar::s::卡}}",
        t1: "{{addvar::s::甲}}",
        t2: "{{addvar::s::乙}}",
        t3: "{{addvar::s::丙}}",
        t4: "{{addvar::s::丁}}",
        t5: "{{addvar::s::戊}}",
        t6: "{{addvar::s::己}}",
      },
    },
  }),
  base("outlet contents get no second pass, quoted or not", [
    e("1", { constant: true, order: 300, position: 7, outletName: "沒人引用", content: "{{getvar::tpl}}" }),
    e("2", { constant: true, order: 200, position: 7, outletName: "有引用", content: "{{getvar::tpl}}" }),
    e("3", { constant: true, order: 100, position: 0, content: "引：{{getvar::q}}" }),
  ], { variables: { local: { tpl: "{{incvar::n}}", q: "{{outlet::有引用}}" } } }),
  base("operation log: values, indexes, same-value indexed writes, deletes", [
    e("1", { constant: true, order: 200, content: "{{setvarindex::arr::1::b}}{{setvarindex::arr::1::b}}{{addvar::cnt::2}}" }),
    e("2", { constant: true, order: 100, content: "{{setglobalvar::g::{{getvar::arr}}}}{{deletevar::cnt}}{{decvar::d}}" }),
  ], { variables: { local: { arr: '["a","b"]', d: 5 } } }),
];
