// 酒館助手的「類巨集」（JS-Slash-Runner 46ec10df `function/macro_like.ts`，只當規格書讀）：送模前在整份提示的
// 每則訊息上、顯示時在訊息上，把 `{{get_<層>_variable::路徑}}` 換成值（字串原樣、其餘 JSON），
// `{{format_<層>_variable::路徑}}` 換成 YAML，多行時每行縮排對齊巨集前面那段字。取值時 `$` 開頭的鍵一律拿掉。
import lodash from "lodash";
import YAML from "yaml";

export type VariableLayer = "message" | "chat" | "character" | "preset" | "global";

/** 各層目前的變數；message 層是最後一則帶變數表的訊息那張 */
export type LayerReader = (layer: VariableLayer) => unknown;

const GET = /\{\{get_(message|chat|character|preset|global)_variable::(.*?)\}\}/gi;
const FORMAT_LINE = /^(.*)\{\{format_(message|chat|character|preset|global)_variable::(.*?)\}\}/im;
const FORMAT = /^(.*)\{\{format_(message|chat|character|preset|global)_variable::(.*?)\}\}/gim;

/** 遞迴拿掉 `$` 開頭的鍵（陣列逐項處理） */
export function withoutDollarKeys(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(withoutDollarKeys);
  if (lodash.isPlainObject(value)) {
    const result: Record<string, unknown> = {};
    for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
      if (!key.startsWith("$")) result[key] = withoutDollarKeys(child);
    }
    return result;
  }
  return value;
}

function valueAt(read: LayerReader, layer: VariableLayer, path: string): unknown {
  // 巨集名不分大小寫，層名照小寫認
  return withoutDollarKeys(lodash.get(read(layer.toLowerCase() as VariableLayer), lodash.unescape(path), null));
}

function formatted(read: LayerReader, prefix: string, layer: VariableLayer, path: string): string {
  // 同一行前面還有別的 format 巨集：先把它換掉（照上游由左而右）
  const earlier = prefix.match(FORMAT_LINE);
  let head = prefix;
  if (earlier) head = formatted(read, earlier[1], earlier[2] as VariableLayer, earlier[3]) + prefix.slice(earlier[0].length);
  const value = valueAt(read, layer, path);
  const text = typeof value === "string" ? value : YAML.stringify(value, { blockQuote: "literal" }).trimEnd();
  return head + text.split("\n").join("\n" + " ".repeat(head.length));
}

export function replaceMacroLike(text: string, read: LayerReader): string {
  if (!text.includes("_variable::")) return text;
  let result = text.replace(GET, (_whole, layer: VariableLayer, path: string) => {
    const value = valueAt(read, layer, path);
    return typeof value === "string" ? value : String(JSON.stringify(value));
  });
  result = result.replace(FORMAT, (_whole, prefix: string, layer: VariableLayer, path: string) => formatted(read, prefix, layer, path));
  return result;
}
