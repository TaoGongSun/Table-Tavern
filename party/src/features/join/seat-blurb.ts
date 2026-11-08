// 選角清單的一行簡介：卡的公開描述常是 Markdown（酒館卡多以標題開頭），這格只有一行，剝成純文字。
const inline = (s: string, name: string) =>
  s
    .replace(/\{\{char\}\}/gi, name)
    .replace(/!\[[^\]]*\]\((?:[^()]|\([^()]*\))*\)/g, "")
    .replace(/\[([^\]]*)\]\((?:[^()]|\([^()]*\))*\)/g, "$1")
    .replace(/<\/?[a-zA-Z][^>]*>/g, "")
    .replace(/(\*{1,3}|`+|~~|__)(.+?)\1/g, "$2")
    .trim();

/** 有內文就略過標題，免得這一行被「簡介」之類的標題佔掉；只有標題時才顯示標題 */
export function seatBlurb(blurb: string, name: string): string {
  const titles: string[] = [];
  const body: string[] = [];
  const visible = blurb.replace(/<(style|script)\b[\s\S]*?(<\/\1>|$)|<!--[\s\S]*?(-->|$)/gi, "");
  let afterText = false; // 上一行是內文（空行、標題之後的 --- 是分隔線）
  for (const line of visible.split("\n")) {
    // 文字緊接下一行全是 = 或 -：那行文字其實是標題
    if (/^\s*(=+|-{2,})\s*$/.test(line) && afterText) {
      titles.push(body.pop()!);
      afterText = false;
      continue;
    }
    afterText = false;
    if (/^\s*(```|~~~|=+\s*$)/.test(line) || /^\s*([-*_]\s*){3,}$/.test(line)) continue;
    const title = /^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$/.exec(line);
    const text = inline(
      title ? title[1] : line.replace(/^\s*(>\s*)+/, "").replace(/^\s*([-+*]|\d+[.)])\s+/, ""),
      name,
    );
    if (text) (title ? titles : body).push(text);
    afterText = !!text && !title;
  }
  return (body.length ? body : titles).join(" ");
}
