// MVU 開局 initvar 的解析與合併（MagVarUpdate 438f9ffc `initvar/variable_init.ts` 的 loadInitVarData、util/common
// parseString，只當規格書讀）：桌面 QuickJS 入口與網頁宿主（web/src/features/mvu/initvar.ts）共用這一份。
import JSON5 from "json5";
import { jsonrepair } from "jsonrepair";
import lodash from "lodash";
import YAML from "yaml";

/** MVU util/common parseString：YAML 優先（`[`／`{` 開頭的改成最後才試），再 JSON5、jsonrepair */
export function parseStructured(content: string): unknown {
  const jsonFirst = /^[[{]/.test(content.trimStart());
  const yaml = () => YAML.parseDocument(content, { merge: true }).toJS();
  if (!jsonFirst) {
    try {
      return yaml();
    } catch {
      // 換下一種
    }
  }
  try {
    return JSON5.parse(content);
  } catch {
    // 換下一種
  }
  try {
    return JSON.parse(jsonrepair(content));
  } catch (error) {
    if (!jsonFirst) throw error;
  }
  return yaml();
}

/** 陣列整個取代、其餘深合併（MVU correctlyMerge） */
export function correctlyMerge<T extends object>(target: T, source: unknown): T {
  return lodash.mergeWith(target, source, (_left: unknown, right: unknown) => (Array.isArray(right) ? right : undefined));
}

/** 條目內容：`<initvar>` 包住或程式碼區塊包住的取裡面那段；剩下的首尾非空行若是成對的 `<標籤>`／`</標籤>` 也剝掉，
 *  不然 YAML 會把首行標籤併進第一個鍵（與桌面版得出同一棵樹） */
export function initvarBody(content: string): string {
  let body = content;
  const xml = body.trim().match(/.*<initvar>.*\n([\s\S]*)\n.*<\/initvar>.*/m);
  if (xml) body = xml[1]!;
  const fenced = body.trim().match(/```.*\n([\s\S]*)\n```/m);
  if (fenced) body = fenced[1]!;
  const wrapped = body.trim().match(/^<([A-Za-z_][\w.-]*)>[ \t]*\n([\s\S]*)\n[ \t]*<\/\1>$/);
  if (wrapped) body = wrapped[2]!;
  return body;
}

/** 開場白裡的 `<initvar>` 區塊：有就整份取代世界書給的初始值（解析失敗的區塊略過）；沒有回 null */
export function openingOverride(opening: string, substitute: (text: string) => string): Record<string, unknown> | null {
  const override: Record<string, unknown> = {};
  let applied = false;
  for (const match of opening.matchAll(/<(initvar)>(?:\s*```.*)?([\s\S]*?)(?:```\s*)?<\/\1>/gim)) {
    try {
      correctlyMerge(override, parseStructured(substitute(match[2]!)));
      applied = true;
    } catch {
      // MVU 記錯誤後略過這一塊
    }
  }
  return applied ? override : null;
}
