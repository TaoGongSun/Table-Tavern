// World Info 觸發：照 SillyTavern 釘版本 06bde939 world-info.js 的 checkWorldInfo（WorldInfoBuffer、
// WorldInfoTimedEffects、filterByInclusionGroups）。設定照 ST 預設（default/content/settings.json）：掃描深度 2、
// 預算 25%、無上限、含名字、遞迴開、不分大小寫、全字比對、不用群組計分（載 MVU 的卡換成 MVU 的推薦值）；
// 最少觸發數與遞迴步數上限都是 0（那兩條路不會走，沒有搬）。網頁版只有卡內世界書（角色書），沒有全域書、對話書、人設書、外部觸發與角色過濾。
// 計時狀態（chat_metadata.timedWorldInfo）以條目穩定 ID 為鍵，計時單位是訊息則數。
import { DEFAULT_DEPTH, DEFAULT_WEIGHT, sortByOrder, WI_LOGIC, WI_POSITION, type WiEntry } from "./world-info-book";

export interface WiSettings {
  depth: number;
  budgetPercent: number;
  budgetCap: number;
  recursive: boolean;
  caseSensitive: boolean;
  matchWholeWords: boolean;
  useGroupScoring: boolean;
  /** 掃描的訊息前面帶「名字: 」 */
  includeNames: boolean;
}

export const ST_WI_SETTINGS: WiSettings = {
  depth: 2,
  budgetPercent: 25,
  budgetCap: 0,
  recursive: true,
  caseSensitive: false,
  matchWholeWords: true,
  useGroupScoring: false,
  includeNames: true,
};

/**
 * 載 MVU 的卡：MVU 開局時把世界書設定換成它的推薦值（MagVarUpdate 438f9ffc `updateLorebookSettings`：
 * 預算 100%、不含名字、不全字比對，其餘同 ST 預設）。D33〔作者裁決 2026-10-07〕
 */
export const MVU_WI_SETTINGS: WiSettings = { ...ST_WI_SETTINGS, budgetPercent: 100, matchWholeWords: false, includeNames: false };
const MAX_SCAN_DEPTH = 1000;
const SCAN = { NONE: 0, INITIAL: 1, RECURSION: 2 } as const;
type ScanState = (typeof SCAN)[keyof typeof SCAN];

export interface WiTimedEffect {
  start: number;
  end: number;
  protected: boolean;
}

/** 存檔 `world_info.timed`：sticky／cooldown 各一張「穩定 ID → 計時」（delay 照 ST 每次由則數算，不存）。 */
export interface WiTimed {
  sticky: Record<string, WiTimedEffect>;
  cooldown: Record<string, WiTimedEffect>;
}

export interface WiGlobalScanData {
  personaDescription: string;
  characterDescription: string;
  characterPersonality: string;
  characterDepthPrompt: string;
  scenario: string;
  creatorNotes: string;
}

export interface WiScanInput {
  /** 要掃的訊息，新到舊（ST chatForWI：`名字: 內容`） */
  chat: string[];
  /** 這次的提示預算（上下文上限－保留輸出）；沒有上限是 Infinity */
  maxContext: number;
  globalScan: WiGlobalScanData;
  /** 生成類型（normal、regenerate…），條目的 triggers 篩這個 */
  trigger: string;
  timed: WiTimed;
  /** substituteParams（巨集副作用照 ST 的先後） */
  substitute: (text: string) => string;
  /** placement 5（世界書）的 regex；depth 只有「依深度插入」的條目才有 */
  regex: (text: string, depth: number | null) => string;
  /** 一段文字幾個 token（ST getTokenCountAsync） */
  countTokens: (text: string) => number;
  random: () => number;
  /** 世界書設定；不給＝ST 預設 */
  settings?: WiSettings;
}

export interface WiDepthGroup {
  depth: number;
  role: number;
  entries: string[];
}

export interface WiResult {
  before: string;
  after: string;
  examples: { position: "before" | "after"; content: string }[];
  depth: WiDepthGroup[];
  anTop: string[];
  anBottom: string[];
  outlets: Record<string, string[]>;
  /** 掃完之後的計時狀態（新的一份，不動傳進來的） */
  timed: WiTimed;
  /** 這次觸發的條目 ID（依加入順序） */
  activated: string[];
}

/** ST parseRegexFromString：只認 `/樣式/旗標`，樣式裡的 `/` 要跳脫。 */
export function parseRegexFromString(input: string): RegExp | null {
  const match = input.match(/^\/([\w\W]+?)\/([gimsuy]*)$/);
  if (!match) return null;
  let [, pattern] = match;
  const flags = match[2];
  if (pattern.match(/(^|[^\\])\//)) return null;
  pattern = pattern.replace("\\/", "/");
  try {
    return new RegExp(pattern, flags);
  } catch {
    return null;
  }
}

const escapeRegex = (text: string) => text.replace(/[/\-\\^$*+?.()|[\]{}]/g, "\\$&");

class WorldInfoBuffer {
  private depthBuffer: (string | undefined)[] = [];
  private recurseBuffer: string[] = [];

  constructor(
    messages: string[],
    private readonly globalScan: WiGlobalScanData,
    private readonly settings: WiSettings,
  ) {
    for (let depth = 0; depth < MAX_SCAN_DEPTH; depth++) {
      if (messages[depth]) this.depthBuffer[depth] = messages[depth].trim();
      if (depth === messages.length - 1) break;
    }
  }

  get(entry: WiEntry): string {
    let depth = entry.scanDepth ?? this.settings.depth;
    if (depth <= 0) return "";
    if (depth > MAX_SCAN_DEPTH) depth = MAX_SCAN_DEPTH;
    const MATCHER = "\x01";
    const JOINER = "\n" + MATCHER;
    let result = MATCHER + this.depthBuffer.slice(0, depth).map((item) => item ?? "").join(JOINER);
    const extra: [boolean, string][] = [
      [entry.matchPersonaDescription, this.globalScan.personaDescription],
      [entry.matchCharacterDescription, this.globalScan.characterDescription],
      [entry.matchCharacterPersonality, this.globalScan.characterPersonality],
      [entry.matchCharacterDepthPrompt, this.globalScan.characterDepthPrompt],
      [entry.matchScenario, this.globalScan.scenario],
      [entry.matchCreatorNotes, this.globalScan.creatorNotes],
    ];
    for (const [enabled, text] of extra) if (enabled && text) result += JOINER + text;
    if (this.recurseBuffer.length > 0) result += JOINER + this.recurseBuffer.join(JOINER);
    return result;
  }

  matchKeys(haystack: string, needle: string, entry: WiEntry): boolean {
    const keyRegex = parseRegexFromString(needle);
    if (keyRegex) return keyRegex.test(haystack);
    const caseSensitive = entry.caseSensitive ?? this.settings.caseSensitive;
    const hay = caseSensitive ? haystack : haystack.toLowerCase();
    const word = caseSensitive ? needle : needle.toLowerCase();
    if (entry.matchWholeWords ?? this.settings.matchWholeWords) {
      if (word.split(/\s+/).length > 1) return hay.includes(word);
      return new RegExp(`(?:^|\\W)(${escapeRegex(word)})(?:$|\\W)`).test(hay);
    }
    return hay.includes(word);
  }

  addRecurse(text: string): void {
    this.recurseBuffer.push(text);
  }

  /** 群組計分：命中的主鍵數，AND_ANY 加次要鍵、AND_ALL 次要鍵全中才加。 */
  getScore(entry: WiEntry): number {
    const text = this.get(entry);
    const primary = entry.key ?? [];
    if (!primary.length) return 0;
    const primaryScore = primary.filter((key) => this.matchKeys(text, key, entry)).length;
    const secondaryScore = entry.keysecondary.filter((key) => this.matchKeys(text, key, entry)).length;
    if (entry.keysecondary.length > 0) {
      if (entry.selectiveLogic === WI_LOGIC.AND_ANY) return primaryScore + secondaryScore;
      if (entry.selectiveLogic === WI_LOGIC.AND_ALL) {
        return secondaryScore === entry.keysecondary.length ? primaryScore + secondaryScore : primaryScore;
      }
    }
    return primaryScore;
  }
}

type TimedType = "sticky" | "cooldown" | "delay";

class TimedEffects {
  private readonly buffer: Record<TimedType, WiEntry[]> = { sticky: [], cooldown: [], delay: [] };

  constructor(
    private readonly chatLength: number,
    private readonly entries: WiEntry[],
    readonly state: WiTimed,
  ) {
    for (const type of ["sticky", "cooldown"] as const) {
      for (const [key, value] of Object.entries(state[type])) {
        if (!value || typeof value !== "object") delete state[type][key];
      }
    }
  }

  private effect(type: "sticky" | "cooldown", entry: WiEntry, isProtected: boolean): WiTimedEffect {
    return { start: this.chatLength, end: this.chatLength + Number(entry[type]), protected: isProtected };
  }

  private checkType(type: "sticky" | "cooldown", onEnded: (entry: WiEntry) => void): void {
    for (const [key, value] of Object.entries(this.state[type])) {
      const entry = this.entries.find((candidate) => candidate.id === key);
      if (this.chatLength <= Number(value.start) && !value.protected) {
        delete this.state[type][key];
        continue;
      }
      if (!entry) {
        if (this.chatLength >= Number(value.end)) delete this.state[type][key];
        continue;
      }
      if (!entry[type]) {
        delete this.state[type][key];
        continue;
      }
      if (this.chatLength >= Number(value.end)) {
        delete this.state[type][key];
        onEnded(entry);
        continue;
      }
      this.buffer[type].push(entry);
    }
  }

  check(): void {
    // sticky 結束時若有 cooldown 就立刻開始冷卻（受保護：對話沒往前也不撤）
    this.checkType("sticky", (entry) => {
      if (!entry.cooldown) return;
      this.state.cooldown[entry.id] = this.effect("cooldown", entry, true);
      this.buffer.cooldown.push(entry);
    });
    this.checkType("cooldown", () => {});
    for (const entry of this.entries) {
      if (entry.delay && this.chatLength < entry.delay) this.buffer.delay.push(entry);
    }
  }

  isActive(type: TimedType, entry: WiEntry): boolean {
    return this.buffer[type].some((candidate) => candidate.id === entry.id);
  }

  set(activated: WiEntry[]): void {
    for (const entry of activated) {
      for (const type of ["sticky", "cooldown"] as const) {
        if (!entry[type]) continue;
        if (!this.state[type][entry.id]) this.state[type][entry.id] = this.effect(type, entry, false);
      }
    }
  }
}

/** filterByInclusionGroups；同一條在多個群組被移除時找不到就不刪（D31，ST 的 splice(indexOf) 會誤刪最後一條）。 */
function filterByInclusionGroups(
  newEntries: WiEntry[],
  allActivated: Map<string, WiEntry>,
  buffer: WorldInfoBuffer,
  timed: TimedEffects,
  random: () => number,
  settings: WiSettings,
): void {
  const grouped: Record<string, WiEntry[]> = {};
  for (const item of newEntries.filter((entry) => entry.group)) {
    for (const group of item.group.split(/,\s*/).filter((name) => name)) (grouped[group] ??= []).push(item);
  }
  if (Object.keys(grouped).length === 0) return;
  const removeEntry = (entry: WiEntry) => {
    const index = newEntries.indexOf(entry);
    if (index !== -1) newEntries.splice(index, 1);
  };
  const removeAllBut = (group: WiEntry[], chosen: WiEntry | null) => {
    for (const entry of group) if (entry !== chosen) removeEntry(entry);
  };

  // 計時：有 sticky 的群組只留 sticky；冷卻與延遲中的移除
  const hasSticky = new Map<string, boolean>();
  for (const [key, group] of Object.entries(grouped)) {
    hasSticky.set(key, false);
    const sticky = group.filter((entry) => timed.isActive("sticky", entry));
    if (sticky.length) {
      for (const entry of group) if (!sticky.includes(entry)) removeEntry(entry);
      hasSticky.set(key, true);
    }
    for (const entry of group.filter((candidate) => timed.isActive("cooldown", candidate))) removeEntry(entry);
    for (const entry of group.filter((candidate) => timed.isActive("delay", candidate))) removeEntry(entry);
  }
  // 群組計分
  for (const [key, group] of Object.entries(grouped)) {
    if (!settings.useGroupScoring && !group.some((entry) => entry.useGroupScoring)) continue;
    if (hasSticky.get(key)) continue;
    const scores = group.map((entry) => buffer.getScore(entry));
    const maxScore = Math.max(...scores);
    for (let index = 0; index < group.length; index++) {
      if (!(group[index].useGroupScoring ?? settings.useGroupScoring)) continue;
      if (scores[index] < maxScore) {
        removeEntry(group[index]);
        group.splice(index, 1);
        scores.splice(index, 1);
        index--;
      }
    }
  }
  for (const [key, group] of Object.entries(grouped)) {
    if (hasSticky.get(key)) continue;
    if ([...allActivated.values()].some((entry) => entry.group === key)) {
      removeAllBut(group, null);
      continue;
    }
    if (group.length <= 1) continue;
    const prios = group.filter((entry) => entry.groupOverride).sort(sortByOrder);
    if (prios.length) {
      removeAllBut(group, prios[0]);
      continue;
    }
    const totalWeight = group.reduce((sum, entry) => sum + (entry.groupWeight ?? DEFAULT_WEIGHT), 0);
    const roll = random() * totalWeight;
    let current = 0;
    let winner: WiEntry | null = null;
    for (const entry of group) {
      current += entry.groupWeight ?? DEFAULT_WEIGHT;
      if (roll <= current) {
        winner = entry;
        break;
      }
    }
    if (winner) removeAllBut(group, winner);
  }
}

/** checkWorldInfo。`entries` 已照 ST 排好；條目內容的巨集代換會改到傳進來的物件，呼叫端要給一份副本。 */
export function checkWorldInfo(entries: WiEntry[], input: WiScanInput): WiResult {
  const timedState: WiTimed = { sticky: { ...input.timed.sticky }, cooldown: { ...input.timed.cooldown } };
  const settings = input.settings ?? ST_WI_SETTINGS;
  const buffer = new WorldInfoBuffer(input.chat, input.globalScan, settings);
  const timed = new TimedEffects(input.chat.length, entries, timedState);
  timed.check();
  const empty: WiResult = { before: "", after: "", examples: [], depth: [], anTop: [], anBottom: [], outlets: {}, timed: timedState, activated: [] };
  if (entries.length === 0) return empty;

  let budget = Math.round((settings.budgetPercent * input.maxContext) / 100) || 1;
  if (settings.budgetCap > 0 && budget > settings.budgetCap) budget = settings.budgetCap;

  const levels = [
    ...new Set(entries.filter((entry) => entry.delayUntilRecursion).map((entry) => (entry.delayUntilRecursion === true ? 1 : Number(entry.delayUntilRecursion)))),
  ].sort((a, b) => a - b);
  let currentLevel = levels.shift() ?? 0;

  let scanState: ScanState = SCAN.INITIAL;
  let overflowed = false;
  const allActivated = new Map<string, WiEntry>();
  const failedProbability = new Set<WiEntry>();
  let allActivatedText = "";

  while (scanState) {
    let nextState: ScanState = SCAN.NONE;
    const activatedNow = new Set<WiEntry>();

    for (const entry of entries) {
      if (failedProbability.has(entry) || allActivated.has(entry.id)) continue;
      if (entry.disable) continue;
      if (entry.triggers.length > 0 && !entry.triggers.includes(input.trigger)) continue;
      const isSticky = timed.isActive("sticky", entry);
      if (timed.isActive("delay", entry)) continue;
      if (timed.isActive("cooldown", entry) && !isSticky) continue;
      if (scanState !== SCAN.RECURSION && entry.delayUntilRecursion && !isSticky) continue;
      if (scanState === SCAN.RECURSION && entry.delayUntilRecursion && Number(entry.delayUntilRecursion) > currentLevel && !isSticky) continue;
      if (scanState === SCAN.RECURSION && settings.recursive && entry.excludeRecursion && !isSticky) continue;
      if (entry.decorators.includes("@@activate")) {
        activatedNow.add(entry);
        continue;
      }
      if (entry.decorators.includes("@@dont_activate")) continue;
      if (entry.constant || isSticky) {
        activatedNow.add(entry);
        continue;
      }
      if (!Array.isArray(entry.key) || !entry.key.length) continue;

      const text = buffer.get(entry);
      const primary = entry.key.find((key) => {
        const substituted = input.substitute(key);
        return substituted && buffer.matchKeys(text, substituted.trim(), entry);
      });
      if (!primary) continue;
      if (!(entry.selective && entry.keysecondary.length)) {
        activatedNow.add(entry);
        continue;
      }
      const logic = entry.selectiveLogic ?? WI_LOGIC.AND_ANY;
      const secondaryMatched = (() => {
        let any = false;
        let all = true;
        for (const key of entry.keysecondary) {
          const substituted = input.substitute(key);
          const hit = !!substituted && buffer.matchKeys(text, substituted.trim(), entry);
          if (hit) any = true;
          else all = false;
          if (logic === WI_LOGIC.AND_ANY && hit) return true;
          if (logic === WI_LOGIC.NOT_ALL && !hit) return true;
        }
        if (logic === WI_LOGIC.NOT_ANY && !any) return true;
        if (logic === WI_LOGIC.AND_ALL && all) return true;
        return false;
      })();
      if (secondaryMatched) activatedNow.add(entry);
    }

    // 機率與預算照這個順序：sticky 先，其餘照排好的順序
    const order = new Map(entries.map((entry, index) => [entry, index]));
    const newEntries =
      activatedNow.size > 1
        ? [...activatedNow].sort(
            (a, b) =>
              Number(timed.isActive("sticky", b)) - Number(timed.isActive("sticky", a)) || (order.get(a) ?? -1) - (order.get(b) ?? -1),
          )
        : [...activatedNow];

    let newContent = "";
    const textTokens = allActivatedText ? input.countTokens(allActivatedText) : 0;
    filterByInclusionGroups(newEntries, allActivated, buffer, timed, input.random, settings);

    let ignoresBudget = newEntries.filter((entry) => entry.ignoreBudget).length;
    for (const entry of newEntries) {
      ignoresBudget -= entry.ignoreBudget ? 1 : 0;
      if (overflowed && !entry.ignoreBudget) {
        if (ignoresBudget > 0) continue;
        break;
      }
      const passes =
        !entry.useProbability || entry.probability === 100 || timed.isActive("sticky", entry) || input.random() * 100 <= entry.probability;
      if (!passes) {
        failedProbability.add(entry);
        continue;
      }
      entry.content = input.substitute(entry.content);
      newContent += `${entry.content}\n`;
      if (!entry.ignoreBudget && textTokens + input.countTokens(newContent) >= budget) {
        overflowed = true;
        continue;
      }
      allActivated.set(entry.id, entry);
    }

    const successful = newEntries.filter((entry) => !failedProbability.has(entry));
    const forRecursion = successful.filter((entry) => !entry.preventRecursion);
    if (settings.recursive && !overflowed && forRecursion.length) nextState = SCAN.RECURSION;
    if (nextState === SCAN.NONE && levels.length) {
      nextState = SCAN.RECURSION;
      currentLevel = levels.shift()!;
    }
    scanState = nextState;
    if (scanState) {
      const text = forRecursion.map((entry) => entry.content).join("\n");
      if (text) {
        buffer.addRecurse(text);
        allActivatedText = text + "\n" + allActivatedText;
      }
    }
  }

  const result: WiResult = { ...empty, examples: [], depth: [], anTop: [], anBottom: [], outlets: {} };
  const before: string[] = [];
  const after: string[] = [];
  for (const entry of [...allActivated.values()].sort(sortByOrder)) {
    const regexDepth = entry.position === WI_POSITION.atDepth ? (entry.depth ?? DEFAULT_DEPTH) : null;
    const content = input.regex(entry.content, regexDepth);
    if (!content) continue;
    switch (entry.position) {
      case WI_POSITION.before:
        before.unshift(content);
        break;
      case WI_POSITION.after:
        after.unshift(content);
        break;
      case WI_POSITION.EMTop:
        result.examples.unshift({ position: "before", content });
        break;
      case WI_POSITION.EMBottom:
        result.examples.unshift({ position: "after", content });
        break;
      case WI_POSITION.ANTop:
        result.anTop.unshift(content);
        break;
      case WI_POSITION.ANBottom:
        result.anBottom.unshift(content);
        break;
      case WI_POSITION.atDepth: {
        const depth = entry.depth ?? DEFAULT_DEPTH;
        const existing = result.depth.find((group) => group.depth === depth && group.role === (entry.role ?? 0));
        if (existing) existing.entries.unshift(content);
        else result.depth.push({ depth: entry.depth, role: entry.role ?? 0, entries: [content] });
        break;
      }
      case WI_POSITION.outlet:
        if (entry.outletName) (result.outlets[entry.outletName] ??= []).push(content);
        break;
    }
  }
  result.before = before.join("\n");
  result.after = after.join("\n");
  timed.set([...allActivated.values()]);
  result.activated = [...allActivated.keys()];
  return result;
}
