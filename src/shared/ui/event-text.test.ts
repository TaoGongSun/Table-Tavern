import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "../../i18n";
import type { TranscriptEvent } from "../contracts/backend-contracts";
import { eventDisplayText, parseMarker, speakerDisplayName } from "./event-text";

function system(marker: unknown, text: string): TranscriptEvent {
  return {
    ts: "",
    speaker_id: "",
    speaker_name: "GM",
    kind: "system",
    text,
    marker: marker as TranscriptEvent["marker"],
  };
}

afterEach(() => setLang("zh-TW"));

describe("parseMarker", () => {
  it("accepts known shapes and rejects unknown or malformed ones", () => {
    expect(parseMarker({ type: "scene_summary" })).toEqual({ type: "scene_summary" });
    expect(parseMarker({ type: "card_arrival", name: "狐狸", extra: 1 })).toEqual({
      type: "card_arrival",
      name: "狐狸",
    });
    expect(parseMarker({ type: "person_arrival", title: "密探" })).toEqual({ type: "person_arrival", title: "密探" });
    for (const bad of [
      undefined,
      null,
      "card_arrival",
      { type: 3 },
      { type: "future_thing", name: "X" },
      { type: "unknown" },
      { type: "card_arrival" },
      { type: "card_arrival", name: 7 },
      { type: "person_arrival", name: "密探" },
      { type: "gm_call" },
    ]) {
      expect(parseMarker(bad)).toBeNull();
    }
  });
});

describe("eventDisplayText", () => {
  it("rebuilds headings, section labels and line breaks like the backend", () => {
    expect(eventDisplayText(system({ type: "card_arrival", name: "狐狸" }, "尾巴很大。"))).toBe(
      "（角色回歸）〈狐狸〉\n公開設定：\n尾巴很大。",
    );
    expect(eventDisplayText(system({ type: "card_arrival", name: "狐狸" }, "  "))).toBe("（角色回歸）〈狐狸〉");
    expect(eventDisplayText(system({ type: "card_private", name: "狐狸" }, "妖狐"))).toBe(
      "（角色私設）〈狐狸〉\n私有設定：\n妖狐",
    );
    expect(eventDisplayText(system({ type: "person_arrival", title: "密探" }, "全文"))).toBe("（人物登場）〈密探〉\n全文");
    expect(eventDisplayText(system({ type: "scene_summary" }, "摘要"))).toBe("【前情提要】\n摘要");
    expect(eventDisplayText(system({ type: "state_update" }, "hp：3"))).toBe("狀態更新\nhp：3");
    expect(eventDisplayText(system({ type: "gm_call", name: "狐狸" }, ""))).toBe("GM 請「狐狸」發言");
    expect(eventDisplayText(system({ type: "gm_call", name: "" }, ""))).toBe("GM 請「玩家」發言");
  });

  it("falls back to the body for unknown or malformed markers", () => {
    expect(eventDisplayText(system({ type: "future_thing" }, "本文"))).toBe("本文");
    expect(eventDisplayText(system({ type: "card_arrival" }, "本文"))).toBe("本文");
    expect(eventDisplayText(system(undefined, "（角色回歸）〈舊〉"))).toBe("（角色回歸）〈舊〉");
  });

  it("follows the current interface language", () => {
    const event = system({ type: "card_arrival", name: "Fox" }, "Big tail.");
    setLang("en");
    expect(eventDisplayText(event)).toBe("(Character returns) “Fox”\nPublic profile:\nBig tail.");
    setLang("fr");
    expect(eventDisplayText(event)).toBe("(Retour d’un personnage) « Fox »\nProfil public :\nBig tail.");
    expect(eventDisplayText(system({ type: "gm_call", name: "" }, ""))).toBe("Le MJ invite « Toi » à parler");
  });
});

describe("speakerDisplayName", () => {
  it("falls back to the player label only for nameless player lines", () => {
    const player = { ...system(undefined, "你好"), kind: "player" as const, speaker_name: "" };
    expect(speakerDisplayName(player)).toBe("玩家");
    setLang("ja");
    expect(speakerDisplayName(player)).toBe("プレイヤー");
    expect(speakerDisplayName({ ...player, speaker_name: "阿濤" })).toBe("阿濤");
    expect(speakerDisplayName({ ...player, kind: "dialogue" })).toBe("");
  });
});
