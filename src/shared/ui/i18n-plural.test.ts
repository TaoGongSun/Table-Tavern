import { afterEach, describe, expect, it } from "vitest";
import { formatMessage, setLang, t } from "../../i18n";
import { expandPlural, messagePlaceholders, parseMessage, widestRendering } from "../../i18n/plural";
import { es } from "../../i18n/es";

afterEach(() => setLang("zh-TW"));

describe("單複數：字典經 t() 展開", () => {
  it("en 1／2", () => {
    setLang("en");
    expect(t("unsavedChanges", { n: 1 })).toBe("1 unsaved change");
    expect(t("unsavedChanges", { n: 2 })).toBe("2 unsaved changes");
    expect(t("unsavedLeaveConfirm", { n: 1 })).toBe(
      "You have 1 unsaved change. Leave anyway? It will be lost.",
    );
  });

  it("ru one／few／many／other 含 0、11、21、22、25 與小數", () => {
    setLang("ru");
    const cases: [number, string][] = [
      [0, "импортировано 0 записей"],
      [1, "импортирована 1 запись"],
      [2, "импортировано 2 записи"],
      [5, "импортировано 5 записей"],
      [11, "импортировано 11 записей"],
      [21, "импортирована 21 запись"],
      [22, "импортировано 22 записи"],
      [25, "импортировано 25 записей"],
      [1.5, "импортировано 1.5 записи"],
    ];
    for (const [n, phrase] of cases) {
      expect(t("worldbookImportDone", { n })).toBe(`В мировую книгу ${phrase}.`);
    }
    expect(t("readOnlySkipped", { count: 2 })).toBe("2 реплики не удалось показать");
  });

  it("pt-BR 0／1 單數、2 複數；fr 0／1 單數", () => {
    setLang("pt-BR");
    expect(t("unsavedChanges", { n: 0 })).toBe("0 alteração não salva");
    expect(t("unsavedChanges", { n: 1 })).toBe("1 alteração não salva");
    expect(t("unsavedChanges", { n: 2 })).toBe("2 alterações não salvas");
    setLang("fr");
    expect(t("unsavedChanges", { n: 0 })).toBe("0 modif non enregistrée");
    expect(t("unsavedChanges", { n: 2 })).toBe("2 modifs non enregistrées");
  });

  it("數字字串照數字判；缺參數、非法值走 other 且照舊代入", () => {
    setLang("en");
    expect(t("cliLoginCooldown", { secs: "1" })).toBe(
      "To avoid duplicate sign-in requests, please retry in 1 second.",
    );
    expect(t("unsavedChanges")).toBe("{n} unsaved changes");
    expect(t("unsavedChanges", {})).toBe("{n} unsaved changes");
    expect(t("unsavedChanges", { n: "abc" })).toBe("abc unsaved changes");
    expect(t("unsavedChanges", { n: Number.NaN })).toBe("NaN unsaved changes");
    const huge = "1".repeat(400);
    expect(t("unsavedChanges", { n: huge })).toBe(`${huge} unsaved changes`);
  });

  it("分支外的一般佔位符照常代入；代入值裡的 {…}／# 不再被解讀", () => {
    expect(formatMessage(es.refactorPartialFailed, "es", { n: 1, names: "{n} # {names}" })).toBe(
      "1 fallido ({n} # {names})",
    );
    expect(formatMessage("{n, plural, one {# x} other {# xs}}", "en", { n: "{n}" })).toBe("{n} xs");
  });

  it("一串多個 plural、分支內含一般佔位符", () => {
    const template =
      "{a, plural, one {# apple for {who}} other {# apples for {who}}} and {b, plural, one {# pear} other {# pears}}";
    expect(formatMessage(template, "en", { a: 1, b: 3, who: "Ann" })).toBe(
      "1 apple for Ann and 3 pears",
    );
    expect(formatMessage(template, "en", { a: 2, b: 1, who: "Bo" })).toBe("2 apples for Bo and 1 pear");
  });
});

describe("plural 文法", () => {
  const bad = {
    缺other: "{n, plural, one {# x}}",
    壞括號: "{n, plural, one {# x} other {# xs}",
    多出右括號: "a } b",
    重複類別: "{n, plural, one {a} one {b} other {c}}",
    未知類別: "{n, plural, single {a} other {b}}",
    等號精確值: "{n, plural, =1 {a} other {b}}",
    offset: "{n, plural, offset:1 one {a} other {b}}",
    巢狀: "{n, plural, one {{m, plural, one {a} other {b}}} other {c}}",
    單引號跳脫: "{n, plural, one {'{a'}} other {b}}",
    其他大括號: "{n, select, a {x} other {y}}",
  };

  it.each(Object.entries(bad))("%s：解析拒絕、執行期原樣回傳", (_, text) => {
    expect(parseMessage(text).ok).toBe(false);
    expect("error" in messagePlaceholders(text)).toBe(true);
    expect(formatMessage(text, "en", { n: 1, who: "Ann", a: "A" })).toBe(text);
  });

  it("壞 plural 的整串連一般佔位符都不代入", () => {
    const text = "{n, plural, one {# item for {who}}}";
    expect(expandPlural(text, "en", { n: 1 })).toBeNull();
    expect(formatMessage(text, "en", { n: 1, who: "Ann" })).toBe(text);
    expect(formatMessage(`{who}: ${text}`, "en", { who: "Ann" })).toBe(`{who}: ${text}`);
  });

  it("沒有 plural 語法的字串照常代入，不受文法檢查影響", () => {
    expect(formatMessage("plural } {who}", "en", { who: "Ann" })).toBe("plural } Ann");
  });

  it("單引號在分支裡是普通字元", () => {
    expect(formatMessage("{n, plural, one {# l'entrée} other {# n'importe}}", "fr", { n: 1 })).toBe(
      "1 l'entrée",
    );
  });
});

describe("字典體檢用的佔位符清單", () => {
  it("plural 參數算一次、分支佔位符算一次，一般佔位符重複照算", () => {
    expect(messagePlaceholders(es.refactorPartialFailed)).toEqual({ names: ["n", "names"] });
    expect(
      messagePlaceholders("{n, plural, one {# x {who}} few {# y {who}} other {# z {who}}}"),
    ).toEqual({ names: ["n", "who"] });
    expect(messagePlaceholders("{from} {to} {from}")).toEqual({ names: ["from", "from", "to"] });
  });

  it("分支佔位符不一致要擋；分支錯參數會與正典不符", () => {
    expect("error" in messagePlaceholders("{n, plural, one {# x {who}} other {# xs}}")).toBe(true);
    expect(messagePlaceholders("{count, plural, one {# x} other {# xs}}")).toEqual({
      names: ["count"],
    });
    expect(messagePlaceholders("{n, plural, one {{n} x} other {# xs {n}}}")).toEqual({
      names: ["n", "n"],
    });
  });

  it("按鈕計寬看最寬分支，不把 ICU 語法算進去", () => {
    const len = (s: string) => s.length;
    expect(widestRendering(es.aiGalleryLoadMore, len)).toBe("Cargar más ({n} restantes)");
    expect(widestRendering("{n, plural, one {# long branch} other {# x}}", len)).toBe("{n} long branch");
  });
});
