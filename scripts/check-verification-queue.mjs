// 待實測清單格式檢查（`npm run verify` 的一步）。
// 規則見 CLAUDE.md「實測在結案前做完」：清單只收這台機器驗不到的，分四段，
// 每列都要寫「測不到原因」與「還沒測」的那一步；寫不出原因的不得排入。
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const FILE = fileURLToPath(new URL("../.ai/reference/verification-queue.md", import.meta.url));
const SECTIONS = ["非作者本人不可", "要真手機", "要 Windows", "要外部條件"];
const FIELDS = ["測不到原因：", "還沒測："];

const errors = [];
const seen = [];
let section = null;
for (const [index, line] of readFileSync(FILE, "utf8").split("\n").entries()) {
  const at = `第 ${index + 1} 行`;
  const heading = /^## (.+)$/.exec(line);
  if (heading) {
    section = heading[1].trim();
    seen.push(section);
    if (!SECTIONS.includes(section)) errors.push(`${at}：段落「${section}」不在四段之內（${SECTIONS.join("、")}）`);
    continue;
  }
  if (section === null || line.trim() === "") continue;
  if (!line.startsWith("- ")) {
    errors.push(`${at}：段落裡只能放「- 」開頭的項目列`);
    continue;
  }
  const parts = line.split("｜");
  for (const field of FIELDS) {
    const part = parts.find((text) => text.trim().startsWith(field));
    if (!part || part.trim().length <= field.length) errors.push(`${at}：缺「${field}」或內容是空的`);
  }
}
for (const name of SECTIONS) if (!seen.includes(name)) errors.push(`缺段落「## ${name}」（沒有項目也要留標題）`);

if (errors.length > 0) {
  console.error(`待實測清單格式不符（${FILE}）：`);
  for (const error of errors) console.error(`  ${error}`);
  console.error("測試通道驗得到的項目要在結案前測完，不排進清單；見 CLAUDE.md「實測在結案前做完」。");
  process.exit(1);
}
console.log("待實測清單格式正確");
