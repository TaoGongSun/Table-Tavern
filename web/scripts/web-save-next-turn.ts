// 網頁存檔端對端對拍（worldbook-st-trigger-parity 方案四之 4）：網頁版讀 web-export-world-info.json、
// 玩家再送一句 `NEXT_USER`，照實送同一支 composePrompt 組下一輪提示——記下哪些條目的內文進了提示（穩定 ID）
// 與掃完的計時表。桌面版匯入同一份存檔、補同一句玩家句，以那個角色的視角掃一次要得到同樣的結果。
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parseWebSave } from "@desktop/shared/contracts/web-save/web-save";
import { composePrompt } from "../src/features/chat/prompt";
import { tableWorldInfo } from "../src/features/chat/world-info-setup";
import { restoreWebSave } from "../src/features/saves/web-save-codec";
import { createChatVariables } from "../src/features/sillytavern/variables";

export const SAVE = "web-export-world-info.json";
export const NEXT_USER = "那我們出發吧，小心狼。";

export function webSaveNextTurn() {
  const path = fileURLToPath(new URL(`../../src/shared/contracts/web-save/${SAVE}`, import.meta.url));
  const parsed = parseWebSave(readFileSync(path, "utf8"));
  if (!parsed.ok) throw new Error(`存檔讀不懂：${JSON.stringify(parsed.error)}`);
  const restored = restoreWebSave(parsed.save);
  if (!restored.ok) throw new Error(`存檔接不回：${JSON.stringify(restored.error)}`);
  const game = restored.game;
  const table = tableWorldInfo(game.card, parsed.save.world_info);
  const entries = [...game.entries, { id: "next-user", role: "user" as const, text: NEXT_USER }];
  const composed = composePrompt(
    { card: game.card, userName: game.userName, variables: createChatVariables(), chatId: "web-next-turn", worldInfo: { entries: table.entries, state: table.state } },
    entries,
    { random: () => 0 },
  );
  const sent = composed.messages.map((message) => message.content).join("\n");
  return {
    save: SAVE,
    nextUser: NEXT_USER,
    // 內文進了提示的條目（穩定 ID，依存檔 entries 的順序）
    inPrompt: table.entries.filter((entry) => entry.content.length > 0 && sent.includes(entry.content)).map((entry) => entry.id),
    timed: composed.worldInfo?.timed ?? null,
  };
}
