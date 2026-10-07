// 穩定免費的目前模型、失敗計數與換模。照桌面版 src-tauri/src/smart_free/failover.rs（Runtime :153、
// record_chat :303、switch :340、pick_for_sentence :101）改寫。網頁版單一分頁、狀態只在記憶體：
// 重新整理後從名單第 1 名重來。票（ticket）仍保留，讓登出、換金鑰後晚到的結果不污染新狀態。
import { countsTowardSwitch, type FailureClass } from "./api-failure";

/** 連續幾次可計數失敗就換模（同桌面版〔作者裁決 2026-10-04〕）。 */
export const FAILURES_TO_SWITCH = 2;
/** 名單全被標成忙線後的冷卻（同桌面版〔作者裁決 2026-10-04〕）。 */
export const EXHAUSTED_COOLDOWN_SECS = 30 * 60;

/** FNV-1a 64：名單或金鑰的穩定指紋（金鑰本身不存進任何狀態）。 */
export function fnv1a(parts: string[]): string {
  let hash = 0xcbf29ce484222325n;
  const encoder = new TextEncoder();
  for (const part of parts) {
    for (const byte of [...encoder.encode(part), 0]) {
      hash ^= BigInt(byte);
      hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
    }
  }
  return hash.toString(16).padStart(16, "0");
}

export interface Ticket {
  account: string;
  epoch: number;
  lineupGen: string;
  currentModel: string;
  selectedModel: string;
  /** 本句因放不下而臨時替代：結果不計入。 */
  substitute: boolean;
}

export type ChatRecord =
  | { kind: "ignored" }
  | { kind: "success" }
  | { kind: "counted" }
  | { kind: "switched"; from: string; to: string }
  /** 名單全在忙線：不換、不重送。 */
  | { kind: "all-busy" };

/** 本句用哪支：目前模型放得下就用它；否則依名單、再依名單外穩定候選找放得下的當替代。 */
export function pickForSentence(
  current: string,
  lineup: string[],
  others: string[],
  fits: (model: string) => boolean,
  exclude: string[],
): { model: string; substitute: boolean } | null {
  const allowed = (model: string) => model !== "" && !exclude.includes(model) && fits(model);
  if (allowed(current)) return { model: current, substitute: false };
  const found = [...lineup, ...others].find((model) => model !== current && allowed(model));
  return found ? { model: found, substitute: true } : null;
}

export class FailoverRuntime {
  account = "";
  lineupGen = "";
  model = "";
  exhausted: string[] = [];
  exhaustedAt: number | null = null;
  /** 選模世代：目前模型、帳號、名單任何一項變動都 +1，計數同步歸零。 */
  epoch = 0;
  count = 0;

  private advance() {
    this.epoch += 1;
    this.count = 0;
  }

  /** 登出或換金鑰：作廢所有在途的票。 */
  invalidate() {
    this.advance();
  }

  /** 取票前對齊帳號與名單、處理冷卻；回傳本句的目前模型。 */
  reconcile(account: string, lineup: string[], now: number): string {
    const gen = fnv1a(lineup);
    if (this.account !== account || this.lineupGen !== gen || !lineup.includes(this.model)) {
      this.account = account;
      this.lineupGen = gen;
      this.model = lineup[0] ?? "";
      this.exhausted = [];
      this.exhaustedAt = null;
      this.advance();
      return this.model;
    }
    if (this.exhaustedAt !== null && now >= this.exhaustedAt + EXHAUSTED_COOLDOWN_SECS) {
      this.exhausted = [];
      this.exhaustedAt = null;
      this.count = 0;
    }
    return this.model;
  }

  ticket(current: string, selected: string, substitute: boolean): Ticket {
    return {
      account: this.account,
      epoch: this.epoch,
      lineupGen: this.lineupGen,
      currentModel: current,
      selectedModel: selected,
      substitute,
    };
  }

  private matches(ticket: Ticket): boolean {
    return (
      ticket.account === this.account &&
      ticket.epoch === this.epoch &&
      ticket.lineupGen === this.lineupGen &&
      ticket.currentModel === this.model
    );
  }

  /** 聊天結果（failure=null 為成功）。只有票對得上、非替代的結果才計入；allowSwitch=false（第 2 發）只記次數。 */
  recordChat(
    ticket: Ticket,
    failure: FailureClass | null,
    allowSwitch: boolean,
    lineup: string[],
    now: number,
  ): ChatRecord {
    if (!this.matches(ticket) || ticket.substitute) return { kind: "ignored" };
    if (failure === null) {
      this.count = 0;
      this.exhausted = [];
      this.exhaustedAt = null;
      return { kind: "success" };
    }
    if (!countsTowardSwitch(failure)) return { kind: "ignored" };
    this.count = failure === "gone" ? Math.max(this.count, FAILURES_TO_SWITCH) : this.count + 1;
    if (!allowSwitch || this.count < FAILURES_TO_SWITCH) return { kind: "counted" };
    return this.switch(lineup, now);
  }

  private switch(lineup: string[], now: number): ChatRecord {
    const from = this.model;
    if (!this.exhausted.includes(from)) this.exhausted.push(from);
    this.exhaustedAt = now;
    const start = Math.max(0, lineup.indexOf(from));
    let target: string | undefined;
    for (let offset = 1; offset <= lineup.length; offset += 1) {
      const candidate = lineup[(start + offset) % lineup.length];
      if (!this.exhausted.includes(candidate)) {
        target = candidate;
        break;
      }
    }
    if (target === undefined) {
      this.count = 0;
      return { kind: "all-busy" };
    }
    this.model = target;
    this.advance();
    return { kind: "switched", from, to: target };
  }
}
