// 數字欄的顯示模板（mechanism.display_formats，例「第 {} 天」）：樹上存數字，畫面與輸入照原卡寫法換算。

// 數字欄照原卡寫法顯示：樹上存數字（觸發要比大小），有模板（「第 {} 天」）就套回去；值不是純數字就原樣
const PLAIN_NUMBER = /^-?\d+(\.\d+)?$/;
export function displayWithFormat(value: string, format: string | undefined): string {
  const trimmed = value.trim();
  return format && PLAIN_NUMBER.test(trimmed) ? format.replace("{}", trimmed) : value;
}

// 反過來：玩家照原寫法輸入（「第 8 天」，空白有無都算）就取出數字存，觸發與增量才讀得到〔作者裁決 2026-10-27〕；
// 不符合模板就原樣（純數字本來就原樣）
export function parseWithFormat(input: string, format: string | undefined): string {
  const slot = format?.indexOf("{}") ?? -1;
  if (!format || slot < 0) return input;
  const squash = (text: string) => text.replace(/\s+/g, "");
  const prefix = squash(format.slice(0, slot));
  const suffix = squash(format.slice(slot + 2));
  const compact = squash(input);
  if (!compact.startsWith(prefix) || !compact.endsWith(suffix) || compact.length <= prefix.length + suffix.length) {
    return input;
  }
  const number = compact.slice(prefix.length, compact.length - suffix.length);
  return PLAIN_NUMBER.test(number) ? number : input;
}

// 顯示前綴（mechanism.display_prefixes，例「👤 」）：原卡狀態欄格子開頭的符號，樹上存不帶符號的值。
// 比對比照後端 rules::rule_for：先精確路徑，再找段數相同、含 * 的鍵（萬用段最少者）
export function prefixFor(prefixes: Record<string, string>, path: string[]): string | undefined {
  const exact = prefixes[path.join(".")];
  if (exact !== undefined) return exact;
  let best: { wild: number; prefix: string } | undefined;
  for (const [key, prefix] of Object.entries(prefixes)) {
    const segments = key.split(".");
    if (segments.length !== path.length || !segments.includes("*")) continue;
    if (!segments.every((segment, index) => segment === "*" || segment === path[index])) continue;
    const wild = segments.filter((segment) => segment === "*").length;
    if (!best || wild < best.wild) best = { wild, prefix };
  }
  return best?.prefix;
}

// 顯示時接回前綴；值已經以它開頭（舊資料或玩家手打）就不再接一次
export function withPrefix(value: string, prefix: string | undefined): string {
  if (!prefix || !value) return value;
  return value.startsWith(prefix.trimEnd()) ? value : prefix + value;
}

// 玩家照原寫法輸入（帶前綴）就剝掉再存
export function stripPrefix(input: string, prefix: string | undefined): string {
  const head = prefix?.trimEnd();
  if (!head || !input.startsWith(head)) return input;
  const rest = input.slice(head.length).trimStart();
  return rest ? rest : input;
}
