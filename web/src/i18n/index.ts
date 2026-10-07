import { zhTW } from "./zh-TW";

export type MsgKey = keyof typeof zhTW;

/** 取文案並代入 `{name}` 佔位；佔位沒給值就原樣留著，方便發現漏傳。 */
export function t(key: MsgKey, vars: Record<string, string | number> = {}): string {
  return zhTW[key].replace(/\{(\w+)\}/g, (whole, name: string) => (name in vars ? String(vars[name]) : whole));
}
