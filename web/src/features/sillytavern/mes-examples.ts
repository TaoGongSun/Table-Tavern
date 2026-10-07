// 範例對話（mes_example）切段與拆訊息，照釘版本 script.js parseMesExamples 與 openai.js
// setOpenAIMessageExamples／parseExampleIntoIndividual（Chat Completion、單人對話）。

/** 依 `<START>` 切段；每段前面補 `<START>\n`、尾端補換行。只有 `<START>` 或空字串＝沒有範例。 */
export function parseMesExamples(examples: string): string[] {
  if (!examples || examples === "<START>") return [];
  const text = examples.startsWith("<START>") ? examples : `<START>\n${examples.trim()}`;
  return text
    .split(/<START>/gi)
    .slice(1)
    .map((block) => `<START>\n${block.trim()}\n`);
}

export interface ExampleMessage {
  role: "system";
  name: "example_user" | "example_assistant";
  content: string;
}

/**
 * 一段範例拆成逐則訊息：第一行（段首標記）略過；`玩家名:` 開頭的行起算玩家、`角色名:` 起算角色，
 * 名字前綴拿掉、去頭尾空白。還沒遇到任何名字前的行會併進下一則。
 */
export function parseExampleIntoIndividual(block: string, userName: string, charName: string): ExampleMessage[] {
  const result: ExampleMessage[] = [];
  const lines = block.split("\n");
  let current: string[] = [];
  let inUser = false;
  let inBot = false;
  const add = (name: string, systemName: ExampleMessage["name"]) => {
    const content = current.join("\n").replace(`${name}:`, "").trim();
    result.push({ role: "system", content, name: systemName });
    current = [];
  };
  for (let index = 1; index < lines.length; index++) {
    const line = lines[index];
    if (line.startsWith(`${userName}:`)) {
      inUser = true;
      if (inBot) add(charName, "example_assistant");
      inBot = false;
    } else if (line.startsWith(`${charName}:`)) {
      inBot = true;
      if (inUser) add(userName, "example_user");
      inUser = false;
    }
    current.push(line);
  }
  if (inUser) add(userName, "example_user");
  else if (inBot) add(charName, "example_assistant");
  return result;
}

/** 所有範例段落 → 每段一組訊息（setOpenAIMessageExamples）。 */
export function exampleDialogues(blocks: string[], userName: string, charName: string): ExampleMessage[][] {
  return blocks.map((block) =>
    parseExampleIntoIndividual(block.replace(/<START>/i, "{Example Dialogue:}").replace(/\r/gm, ""), userName, charName),
  );
}
