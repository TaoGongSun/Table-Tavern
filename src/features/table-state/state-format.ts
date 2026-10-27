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
