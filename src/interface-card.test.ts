import { describe, expect, it } from "vitest";
import { applyScripts, buildShellDocument, extractShell, parseStRegex, type InterfaceScript } from "./interface-card";

function makeScript(overrides: Partial<InterfaceScript>): InterfaceScript {
  return {
    name: "test",
    find_regex: "",
    replace_string: "",
    trim_strings: [],
    min_depth: null,
    max_depth: null,
    ...overrides,
  };
}

describe("parseStRegex", () => {
  it("解析 /樣式/旗標 形式並保留 dotAll", () => {
    const regex = parseStRegex("/foo.bar/s");
    expect(regex).not.toBeNull();
    expect(regex?.flags).toBe("s");
    expect(regex?.test("foo\nbar")).toBe(true);
  });

  it("裸樣式（無包夾斜線）可直接編譯使用", () => {
    const regex = parseStRegex("\\s*请选择你的身份\\s*");
    expect(regex).not.toBeNull();
    expect(regex?.test("  请选择你的身份  ")).toBe(true);
  });

  it("丟掉 JS 不認得的旗標，其餘旗標照常生效", () => {
    const regex = parseStRegex("/abc/gx");
    expect(regex).not.toBeNull();
    expect(regex?.flags).toBe("g");
    expect(regex?.test("abc")).toBe(true);
  });

  it("壞樣式回 null，不丟例外", () => {
    expect(parseStRegex("/[/")).toBeNull();
  });
});

describe("applyScripts", () => {
  it("套用西幻卡真實 find_regex：$1/$2 換入、$& 與 $( 不被 JS 語意吃掉", () => {
    const script = makeScript({
      find_regex:
        "/<GoldenRPG_UI>.*?<CurrentView>([\\s\\S]*?)<\\/CurrentView>.*?<WorldSystem>([\\s\\S]*?)<\\/WorldSystem>.*?<\\/GoldenRPG_UI>/s",
      replace_string: "```html\n<!DOCTYPE html><body><div>$1</div><div>$2</div><span>$&與$(jQuery)</span></body>\n```",
    });
    const raw = "<GoldenRPG_UI><CurrentView>正文</CurrentView><WorldSystem>世界</WorldSystem></GoldenRPG_UI>";

    const result = applyScripts(raw, [script]);

    expect(result).toContain("<div>正文</div>");
    expect(result).toContain("<div>世界</div>");
    expect(result).toContain("$&與$(jQuery)");
  });

  it("{{match}} 換成整段命中文字；trim_strings 先從命中文字挖掉指定片段", () => {
    const script = makeScript({
      find_regex: "\\[(.*?)\\]",
      replace_string: "before {{match}} after, group=$1",
      trim_strings: ["DROPME"],
    });
    const raw = "trim me: [KEEP-DROPME]";

    const result = applyScripts(raw, [script]);

    expect(result).toBe("trim me: before [KEEP-] after, group=KEEP-");
    expect(result).not.toContain("DROPME");
  });

  it("中間一支腳本樣式壞掉時只跳過該支，前後兩支照常套用", () => {
    const scripts = makeThreeScriptsWithBadMiddle();

    const result = applyScripts("AAA", scripts);

    expect(result).toBe("CCC");
  });
});

function makeThreeScriptsWithBadMiddle(): InterfaceScript[] {
  return [
    makeScript({ find_regex: "/A/g", replace_string: "B" }),
    makeScript({ find_regex: "/[/", replace_string: "永遠用不到" }),
    makeScript({ find_regex: "/B/g", replace_string: "C" }),
  ];
}

describe("extractShell", () => {
  it("抽出 ```html 圍欄內的內容", () => {
    const rendered = "前言文字\n```html\n<!DOCTYPE html><html><body>介面</body></html>\n```\n後記";

    expect(extractShell(rendered)).toBe("<!DOCTYPE html><html><body>介面</body></html>");
  });

  it("沒有圍欄但找得到 <!DOCTYPE html> 時，從該處取到字串結尾", () => {
    const rendered = "模型先講了一段話。<!DOCTYPE html><html><body>介面</body></html>";

    expect(extractShell(rendered)).toBe("<!DOCTYPE html><html><body>介面</body></html>");
  });

  it("純文字找不到殼時回 null", () => {
    expect(extractShell("這裡只有純文字，沒有任何 HTML 介面")).toBeNull();
  });
});

describe("buildShellDocument", () => {
  it("輸出含誘餌 textarea、__ttHost 橋接物件與 triggerSlash", () => {
    const doc = buildShellDocument("<html><head></head><body>殼內容</body></html>");

    expect(doc).toContain("send_textarea");
    expect(doc).toContain("__ttHost");
    expect(doc).toContain("triggerSlash");
  });

  it("殼裡的 window.parent.document 字面會被改寫成 window.__ttHost.document", () => {
    const shell = "<html><head></head><body><script>window.parent.document.getElementById('x')</script></body></html>";

    const doc = buildShellDocument(shell);

    expect(doc).toContain("window.__ttHost.document.getElementById('x')");
    expect(doc).not.toContain("window.parent.document.getElementById('x')");
  });
});
