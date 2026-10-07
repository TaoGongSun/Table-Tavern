import { describe, expect, it } from "vitest";
import type { ChatEntry } from "../chat/chat-turn";
import { humanizedDateTime, toStChat } from "./st-chat";

const T0 = Date.parse("2026-10-07T21:00:00Z");

describe("匯出成 SillyTavern 聊天檔", () => {
  const entries: ChatEntry[] = [
    { id: "m0", role: "char", text: "開場 {{user}}", opening: true, sentAt: T0 },
    { id: "m1", role: "user", text: "你好", sentAt: T0 + 1000 },
    { id: "m2", role: "char", text: "嗨", raw: "原文", interrupted: true, vars: { x: 1 } },
  ];
  const text = toStChat({ userName: "旅人", characterName: "瑟拉", texts: ["開場 旅人"], entries, createdAt: T0 + 5000 });
  const lines = text.split("\n").map((line) => JSON.parse(line) as Record<string, unknown>);

  it("一行一則 JSON，第一行是 ST 認得的檔頭", () => {
    expect(lines).toHaveLength(4);
    expect(lines[0]).toEqual({
      user_name: "旅人",
      character_name: "瑟拉",
      create_date: humanizedDateTime(T0 + 5000),
      chat_metadata: {},
    });
  });

  it("每則照 ST 訊息形狀；開場白用代換好的字；只帶對話（變數、原文、中斷旗標都不帶）", () => {
    expect(lines.slice(1)).toEqual([
      { name: "瑟拉", is_user: false, is_system: false, send_date: "2026-10-07T21:00:00.000Z", mes: "開場 旅人", extra: {} },
      { name: "旅人", is_user: true, is_system: false, send_date: "2026-10-07T21:00:01.000Z", mes: "你好", extra: {} },
      { name: "瑟拉", is_user: false, is_system: false, send_date: "2026-10-07T21:00:05.000Z", mes: "嗨", extra: {} },
    ]);
  });

  it("humanizedDateTime 照 ST 的格式", () => {
    expect(humanizedDateTime(new Date(2026, 0, 2, 3, 4, 5, 6).getTime())).toBe("2026-01-02@03h04m05s006ms");
  });
});
