import { describe, expect, it } from "vitest";
import cases from "../../shared/contracts/reply-cleanup/cases.json";
import { streamDisplayText } from "./stream-display";

const TAG_OPENER = /<updatevariable|<status|<details/i;

/** 逐 chunk 送：每一步畫面上的字 */
function frames(text: string, size = 1): string[] {
  const shown: string[] = [];
  for (let end = size; end < text.length + size; end += size) shown.push(streamDisplayText(text.slice(0, end)));
  return shown;
}

describe("streamDisplayText", () => {
  it.each(cases.cases)("共用案例 $name", ({ input, stream }) => {
    expect(streamDisplayText(input)).toBe(stream);
    // 串流畫面不出現任何控制標籤開頭（後端最終文字保留的只會是正文或已閉合的非狀態 details）
    expect(TAG_OPENER.test(streamDisplayText(input))).toBe(false);
  });

  it("開標籤→內容→閉標籤→後續正文，逐字與跨 chunk 送都不閃出標籤", () => {
    const text = "她點頭。<UpdateVariable>{\"a\":1}</UpdateVariable>\n又說了一句。";
    for (const size of [1, 3, 7]) {
      for (const frame of frames(text, size)) {
        expect(frame).not.toMatch(/</);
        expect("她點頭。".startsWith(frame)).toBe(true);
      }
    }
  });

  it("GM 串流途中狀態區塊已閉合仍不顯示", () => {
    expect(streamDisplayText("夜深了。<status>hp: 3</status>雨停了")).toBe("夜深了。");
    expect(streamDisplayText("夜深了。\n```state\nhp: 3\n```\n雨停了")).toBe("夜深了。\n");
  });

  it("角色「標籤＋換行＋正文」串流中不顯示標籤", () => {
    for (const frame of frames("<UpdateVariable>x</UpdateVariable>\n你好")) {
      expect(frame).toBe("");
    }
  });

  it("<maintext> 外殼不顯示、正文逐步顯示", () => {
    const shown = frames("<maintext>夜深了</maintext>");
    expect(shown.every((frame) => !frame.includes("<"))).toBe(true);
    expect(shown).toContain("夜");
    expect(shown).toContain("夜深了");
    expect(shown[shown.length - 1]).toBe("夜深了");
  });

  it("確定不是標籤的 < 在後續字到時顯示", () => {
    expect(streamDisplayText("a <")).toBe("a ");
    expect(streamDisplayText("a < b")).toBe("a < b");
    expect(streamDisplayText("真的<Up")).toBe("真的");
    expect(streamDisplayText("真的<Upside")).toBe("真的<Upside");
  });
});
