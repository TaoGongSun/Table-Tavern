// ST 聊天變數（local＝這段對話、global＝跨對話），語意照釘版本 public/scripts/variables.js：
// 讀出來是數字字串就轉數字、加法遇到非數字改成字串串接、JSON 陣列就 push、帶 index 存成 JSON。

export type VariableMap = Record<string, unknown>;

export interface IndexArgs {
  index?: string;
}

function parseOrNull(text: unknown): unknown {
  try {
    return JSON.parse(text as string) as unknown;
  } catch {
    return undefined;
  }
}

export class VariableScope {
  /** 這個範圍寫過或刪過的鍵（寫回原值也算，寫入失敗不算）：存檔匯出時判斷這桌碰過哪些 global 鍵 */
  readonly written = new Set<string>();

  constructor(readonly values: VariableMap) {}

  has(name: string): boolean {
    return this.values[name] !== undefined;
  }

  get(name: string, args: IndexArgs = {}): unknown {
    let value = this.values[name];
    if (args.index !== undefined) {
      const parsed = parseOrNull(value);
      if (parsed !== undefined && parsed !== null) {
        const index = Number(args.index);
        value = (parsed as Record<string, unknown>)[Number.isNaN(index) ? args.index : index];
        if (typeof value === "object") value = JSON.stringify(value);
      }
    }
    const trimmed = typeof value === "string" ? value.trim() : undefined;
    return trimmed === "" || Number.isNaN(Number(value)) ? value || "" : Number(value);
  }

  set(name: string, value: unknown, args: IndexArgs = {}): unknown {
    if (!name) throw new Error("Variable name cannot be empty or undefined.");
    if (args.index !== undefined) {
      try {
        let parsed = JSON.parse((this.values[name] as string | undefined) ?? "null") as Record<string, unknown> | unknown[] | null;
        const index = Number(args.index);
        if (Number.isNaN(index)) {
          if (parsed === null) parsed = {};
          (parsed as Record<string, unknown>)[args.index] = value;
        } else {
          if (parsed === null) parsed = [];
          (parsed as unknown[])[index] = value;
        }
        this.values[name] = JSON.stringify(parsed);
        this.written.add(name);
      } catch {
        // 跟 ST 一樣：存不進去就算了
      }
    } else {
      this.values[name] = value;
      this.written.add(name);
    }
    return value;
  }

  del(name: string): string {
    this.written.add(name);
    delete this.values[name];
    return "";
  }

  add(name: string, value: unknown): unknown {
    const current = this.get(name) || 0;
    const parsed = parseOrNull(current);
    if (Array.isArray(parsed)) {
      parsed.push(value);
      this.set(name, JSON.stringify(parsed));
      return parsed;
    }
    const increment = Number(value);
    if (Number.isNaN(increment) || Number.isNaN(Number(current))) {
      const text = String(current || "") + String(value);
      this.set(name, text);
      return text;
    }
    const next = Number(current) + increment;
    if (Number.isNaN(next)) return "";
    this.set(name, next);
    return next;
  }

  inc(name: string): unknown {
    return this.add(name, 1);
  }

  dec(name: string): unknown {
    return this.add(name, -1);
  }
}

export interface ChatVariables {
  local: VariableScope;
  global: VariableScope;
}

export function createChatVariables(local: VariableMap = {}, global: VariableMap = {}): ChatVariables {
  return { local: new VariableScope(local), global: new VariableScope(global) };
}

/** 變數副本：試組提示用，副作用不落到原本的變數。 */
export function copyVariables(variables: ChatVariables): ChatVariables {
  return createChatVariables(structuredClone(variables.local.values), structuredClone(variables.global.values));
}

/**
 * 一個回合的提交紀錄：`since`＝這回合上次提交後 target 的樣子（還沒提交＝回合開頭），`outside`＝這回合裡被別處
 * （卡片介面）改過的鍵。外部改過的鍵整個回合都歸外部，之後每一發的提交都不再碰。
 */
export interface TurnCommits {
  since: ChatVariables;
  outside: { local: Set<string>; global: Set<string> };
}

export function turnCommits(target: ChatVariables): TurnCommits {
  return { since: copyVariables(target), outside: { local: new Set(), global: new Set() } };
}

/**
 * 把副本的內容整份寫回 `target`（原地改：global 是跨對話共用的同一個物件），副本寫過的鍵併進來。給了回合的提交
 * 紀錄就三方合併：target 從上次提交後被別處改過的鍵（新增、改值、刪除）記成外部的，連同這回合先前記下的，都保留
 * target 現在的值，其餘照副本。
 */
export function commitVariables(target: ChatVariables, source: ChatVariables, turn?: TurnCommits): void {
  for (const name of source.local.written) target.local.written.add(name);
  for (const name of source.global.written) target.global.written.add(name);
  const scopes = [
    { into: target.local.values, from: source.local.values, before: turn?.since.local.values, outside: turn?.outside.local },
    { into: target.global.values, from: source.global.values, before: turn?.since.global.values, outside: turn?.outside.global },
  ];
  for (const { into, from, before, outside } of scopes) {
    const next = structuredClone(from);
    if (before !== undefined && outside !== undefined) {
      for (const key of new Set([...Object.keys(before), ...Object.keys(into)])) {
        if (JSON.stringify(into[key]) !== JSON.stringify(before[key])) outside.add(key);
      }
      for (const key of outside) {
        if (key in into) next[key] = structuredClone(into[key]);
        else delete next[key];
      }
    }
    for (const key of Object.keys(into)) delete into[key];
    Object.assign(into, next);
  }
  if (turn) turn.since = copyVariables(target);
}
