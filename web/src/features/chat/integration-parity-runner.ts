// 巨集×世界書掃描的跨引擎對拍（worldbook-st-trigger-parity 包 5b，方案四之 1 `integration-cases`）：
// 照 prompt.ts composePrompt 的先後跑「卡欄位第一輪 → 世界書掃描（代換鍵與內文，讀上一輪 outlet）→ outlet 換成本輪 →
// 世界書前、後 → 卡欄位第二輪 → 注入段落（由淺到深）」，回傳中間產物與變數寫入序列（不比整份提示，兩邊組法本來就
// 不同）。案例檔在 src/shared/contracts/world-info/integration-cases.json，預期值由 web/scripts/gen-world-info-fixtures.mjs
// 產生。桌面版的卡只有公開設定一段（P7：描述、個性、角色深度提示都是它），所以案例的卡只給 description，其餘欄位照
// 同一段，第一輪只跑一次（網頁版照 ST 對三個欄位各跑一次）；卡欄位巨集照桌面版回傳第一輪的結果、不再執行副作用。
// 這兩點是對拍的正規化契約（照桌面版語意，不是 ST 行為，方案四之 1、七）——案例裡被卡欄位巨集引用的卡，第一輪
// 產物不含巨集，所以兩種做法的文字相同。
import type { JsonObject } from "../cards/card-file";
import type { ChatLine } from "../sillytavern/macro-engine";
import { baseChatReplace, substituteParams, type CardText, type MacroContext } from "../sillytavern/substitute";
import { VariableScope, type IndexArgs, type VariableMap } from "../sillytavern/variables";
import { resolveEntry, sortByOrder, type WiEntry } from "../sillytavern/world-info-book";
import { checkWorldInfo, ST_WI_SETTINGS } from "../sillytavern/world-info-scan";
import { authorsNotePrompt, injectExtensionPrompts, roleName, type ExtensionPrompt } from "./injections";

export interface IntegrationCase {
  name: string;
  /** 卡名與公開設定（桌面版的 public_md） */
  card: { name: string; description: string };
  userName: string;
  /** 舊到新 */
  chat: ChatLine[];
  variables?: { local?: VariableMap; global?: VariableMap };
  /** 上一輪實送記下的 outlet */
  prevOutlets?: Record<string, string>;
  /** 物件形條目（id 是桌面版的 uid） */
  entries: { id: string; raw: JsonObject }[];
}

export interface IntegrationExpected {
  /** 觸發條目掃描時代換過的內文，依 id 排序 */
  scanned: [string, string][];
  /** 世界書前、後再代換一次的結果 */
  before: string;
  after: string;
  /** 作者註記與依深度條目（深的在前，同深度 assistant → user → system），代換、trim 過 */
  injections: string[];
  /** 本輪 outlet：依名稱排序的 [名稱, 內容] */
  outlets: [string, string][];
  /** 跑完的變數表，`JSON.stringify` 原文 */
  local: string;
  global: string;
  /**
   * 真的寫成的變數操作，依發生順序：`<local|global>:set:<名稱>[<index>]=<值>`、`…:add:<名稱>=<加數>`（incvar／decvar
   * 算 add）、`…:del:<名稱>`；值與加數是 `JSON.stringify` 原文（桌面版操作序列同一格式）
   */
  ops: string[];
}

const json = (value: unknown) => JSON.stringify(value) ?? "undefined";

/** 記下每次真的寫成的操作（add 內部的 set 不另記；寫回同一個值也算寫成）。 */
class RecordingScope extends VariableScope {
  private adding = 0;
  private addWrote = false;
  private readonly scope: "local" | "global";
  private readonly log: string[];

  constructor(values: VariableMap, scope: "local" | "global", log: string[]) {
    super(values);
    this.scope = scope;
    this.log = log;
  }

  override set(name: string, value: unknown, args: IndexArgs = {}): unknown {
    // written 只在真的寫成時加這個鍵：先拿掉再看，帶 index 寫進同樣的 JSON 也認得出
    const had = this.written.has(name);
    this.written.delete(name);
    const result = super.set(name, value, args);
    const wrote = this.written.has(name);
    if (had) this.written.add(name);
    if (!wrote) return result;
    if (this.adding) this.addWrote = true;
    else this.log.push(`${this.scope}:set:${name}${args.index === undefined ? "" : `[${args.index}]`}=${json(value)}`);
    return result;
  }

  override add(name: string, value: unknown): unknown {
    this.adding += 1;
    this.addWrote = false;
    try {
      return super.add(name, value);
    } finally {
      this.adding -= 1;
      if (this.addWrote) this.log.push(`${this.scope}:add:${name}=${json(value)}`);
    }
  }

  override del(name: string): string {
    this.log.push(`${this.scope}:del:${name}`);
    return super.del(name);
  }
}

const CHAT_ID = "table";

export function runIntegrationCase(item: IntegrationCase): IntegrationExpected {
  const card: CardText = {
    name: item.card.name,
    description: item.card.description,
    personality: item.card.description,
    scenario: "",
    first_mes: "",
    mes_example: "",
    creator_notes: "",
    system_prompt: "",
    post_history_instructions: "",
    alternate_greetings: [],
    character_version: "",
    depth_prompt: item.card.description,
  };
  const ops: string[] = [];
  const variables = {
    local: new RecordingScope(structuredClone(item.variables?.local ?? {}), "local", ops),
    global: new RecordingScope(structuredClone(item.variables?.global ?? {}), "global", ops),
  };
  const last = item.chat[item.chat.length - 1];
  let context: MacroContext = {
    card,
    userName: item.userName,
    chat: item.chat,
    variables,
    chatId: CHAT_ID,
    input: last?.isUser ? last.text : "",
    generationType: "normal",
    model: "",
    random: () => 0,
    outlets: item.prevOutlets ?? {},
  };
  const fill = (text: string) => substituteParams(text, context);
  // 1. 卡欄位第一輪（只有公開設定一段；桌面版的 own_base）。之後的卡欄位巨集回傳這次的結果：這是對拍的正規化
  //    契約（照桌面版語意改寫 context.card，不是 ST 行為；ST 每用一次重算一次，方案七），案例裡被引用的卡第一輪產物
  //    不含巨集，所以重算出的文字相同、只差副作用次數
  const description = baseChatReplace(card.description.trim(), context);
  context = { ...context, card: { ...card, description, personality: description, depth_prompt: description } };
  // 2. 掃描
  const entries: WiEntry[] = item.entries
    .map(({ id, raw }) => {
      const { fields, decorators } = resolveEntry(raw, true);
      return { ...fields, id, bookKey: id, decorators };
    })
    .sort(sortByOrder);
  const wi = checkWorldInfo(entries, {
    settings: ST_WI_SETTINGS,
    chat: item.chat.map((line) => `${line.isUser ? item.userName : card.name}: ${line.text}`).reverse(),
    maxContext: Number.POSITIVE_INFINITY,
    globalScan: {
      personaDescription: "",
      characterDescription: description,
      characterPersonality: description,
      characterDepthPrompt: description,
      scenario: "",
      creatorNotes: "",
    },
    trigger: "normal",
    timed: { sticky: {}, cooldown: {} },
    substitute: (text) => fill(text),
    regex: (text) => text,
    countTokens: () => 0,
    random: () => 0,
  });
  const scanned = wi.activated
    .map((id): [string, string] => [id, entries.find((entry) => entry.id === id)?.content ?? ""])
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  // 3. 掃完：outlet 換成本輪值，之後的代換讀它
  const outlets = Object.fromEntries(Object.entries(wi.outlets).map(([name, values]) => [name, values.join("\n")]));
  context = { ...context, outlets };
  // 4. 世界書前／後 → 卡欄位第二輪（prompt.ts 的 wiBefore、wiAfter、descriptionText）→ 注入段落
  const before = fill(wi.before);
  const after = fill(wi.after);
  fill(description);
  const injections: ExtensionPrompt[] = [
    authorsNotePrompt(wi.anTop, wi.anBottom),
    ...wi.depth.map((group) => ({
      key: `customDepthWI_${group.depth}_${group.role}`,
      value: group.entries.join("\n"),
      depth: group.depth,
      role: roleName(group.role),
    })),
  ];
  const injected = injectExtensionPrompts([], injections, fill).map((message) => message.content);
  return {
    scanned,
    before,
    after,
    injections: injected,
    outlets: Object.entries(outlets).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)),
    local: JSON.stringify(variables.local.values),
    global: JSON.stringify(variables.global.values),
    ops,
  };
}
