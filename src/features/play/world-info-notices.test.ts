// 世界書待回報：每則提示一次、按確認才刪；確認沒送成就留著下次再提示；讀不到不擋。
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
const messageMock = vi.hoisted(() => vi.fn(async (_text: string, _options?: unknown) => undefined));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ message: messageMock }));

import { noticeText, showWorldInfoNotices, type WorldInfoNotice } from "./world-info-notices";

const notice = (id: string, layer: string, reason: WorldInfoNotice["reason"]): WorldInfoNotice => ({
  id,
  turn_key: "t1",
  layer,
  reason,
});

describe("world info notices", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    messageMock.mockClear();
  });

  it("shows each notice once and acks it", async () => {
    const notices = [notice("t1:0", "chat", "overwritten"), notice("t1:1", "global", "unknown")];
    invokeMock.mockImplementation(async (command: string) => (command === "world_info_notices" ? notices : undefined));
    await showWorldInfoNotices("w1");
    expect(messageMock).toHaveBeenCalledTimes(2);
    expect(messageMock.mock.calls[0][0]).toBe(noticeText(notices[0]));
    expect(invokeMock).toHaveBeenCalledWith("ack_world_info_notice", { worldId: "w1", id: "t1:0" });
    expect(invokeMock).toHaveBeenCalledWith("ack_world_info_notice", { worldId: "w1", id: "t1:1" });
  });

  it("keeps the notice when the ack fails and ignores read failures", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "world_info_notices") return [notice("t1:0", "chat", "unknown")];
      throw new Error("ack failed");
    });
    await expect(showWorldInfoNotices("w1")).resolves.toBeUndefined();
    invokeMock.mockReset();
    invokeMock.mockRejectedValue(new Error("unreadable"));
    await expect(showWorldInfoNotices("w1")).resolves.toBeUndefined();
  });

  it("names the layer in the text", () => {
    expect(noticeText(notice("a", "chat", "overwritten"))).not.toBe(noticeText(notice("a", "global", "overwritten")));
    expect(noticeText(notice("a", "chat", "overwritten"))).not.toBe(noticeText(notice("a", "chat", "unknown")));
  });
});
