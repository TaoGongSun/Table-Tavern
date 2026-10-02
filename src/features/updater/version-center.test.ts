import { afterEach, describe, expect, it } from "vitest";
import { setLang } from "../../i18n";
import type { UpdateOffer } from "./useUpdateController";
import type { VersionRow } from "./useVersionStoreController";
import {
  cannotReplace,
  formatBytes,
  previousRow,
  rollbackDialogText,
  rollbackNotice,
  showUpdateDot,
  startupReminder,
  updateChanged,
  versionRowActions,
} from "./version-center";

// 後端錯誤代碼（ui_msg.rs 的 UiMsg）。
const SIGNATURE_INVALID = 'TTMSG:{"code":"signature_invalid"}';
const PLATFORM_MISMATCH = 'TTMSG:{"code":"rollback_platform_mismatch"}';
const CANNOT_REPLACE = 'TTMSG:{"code":"update_cannot_replace"}';
const UPDATE_CHANGED = 'TTMSG:{"code":"update_changed"}';

const offer = (level: UpdateOffer["level"]): UpdateOffer => ({
  version: "0.3.0",
  current_version: "0.2.0",
  notes: null,
  pub_date: null,
  level,
  skipped: false,
});

const row = (patch: Partial<VersionRow>): VersionRow => ({
  version: "0.1.0",
  size: 1,
  format_version: 1,
  usable: true,
  eligible: false,
  current: false,
  previous: false,
  ...patch,
});

describe("startupReminder", () => {
  it("功能版出橫幅、格式轉換出對話框、小修只亮點", () => {
    expect(startupReminder(offer("feature"), {})).toBe("banner");
    expect(startupReminder(offer("format"), {})).toBe("dialog");
    expect(startupReminder(offer("patch"), {})).toBeNull();
    expect(startupReminder(null, {})).toBeNull();
  });

  it("被略過或這版已提醒過都不出，別版的紀錄不算", () => {
    for (const level of ["feature", "format", "patch"] as const) {
      expect(startupReminder(offer(level), { update_skipped_version: "0.3.0" })).toBeNull();
      expect(startupReminder(offer(level), { update_reminded_version: "0.3.0" })).toBeNull();
    }
    expect(startupReminder(offer("feature"), { update_reminded_version: "0.2.9" })).toBe("banner");
    expect(startupReminder(offer("format"), { update_skipped_version: "0.2.9" })).toBe("dialog");
  });
});

describe("showUpdateDot", () => {
  it("有沒被略過的新版就亮，略過或沒有就熄", () => {
    expect(showUpdateDot(offer("patch"), {})).toBe(true);
    expect(showUpdateDot(offer("patch"), { update_skipped_version: "0.3.0" })).toBe(false);
    expect(showUpdateDot(null, {})).toBe(false);
  });
});

describe("version rows", () => {
  it("目前那列不給動作；非目前列都能刪，eligible 的另給回退；驗不過的只給刪除", () => {
    expect(versionRowActions(row({ current: true, eligible: true }))).toEqual({
      current: true,
      rollback: false,
      delete: false,
    });
    expect(versionRowActions(row({ eligible: true, previous: true }))).toEqual({
      current: false,
      rollback: true,
      delete: true,
    });
    expect(versionRowActions(row({ usable: false }))).toEqual({
      current: false,
      rollback: false,
      delete: true,
    });
    expect(versionRowActions(row({ usable: true, eligible: false }))).toEqual({
      current: false,
      rollback: false,
      delete: true,
    });
  });

  it("一鍵上一版只認 previous 而且 eligible 的那列", () => {
    expect(previousRow([row({ previous: true, eligible: false })])).toBeNull();
    expect(previousRow([row({ previous: false, eligible: true })])).toBeNull();
    expect(previousRow([row({ previous: true, eligible: true })])?.version).toBe("0.1.0");
    expect(previousRow(undefined)).toBeNull();
  });
});

describe("rollbackNotice", () => {
  const world = (name: string) => ({ id: name, name });

  it("目標不合格只給原因", () => {
    expect(rollbackNotice({ kind: "invalid", message: SIGNATURE_INVALID })).toEqual({
      kind: "invalid",
      message: SIGNATURE_INVALID,
    });
  });

  it("先判 scan_failed，再判兩組是否都空", () => {
    expect(
      rollbackNotice({
        kind: "preview",
        preview: { will_be_readonly: [world("霧港")], maybe_readonly: [], scan_failed: true },
      }),
    ).toEqual({ kind: "scanFailed" });
    expect(
      rollbackNotice({
        kind: "preview",
        preview: { will_be_readonly: [], maybe_readonly: [], scan_failed: false },
      }),
    ).toEqual({ kind: "none" });
    expect(
      rollbackNotice({
        kind: "preview",
        preview: {
          will_be_readonly: [world("霧港")],
          maybe_readonly: [world("讀不懂")],
          scan_failed: false,
        },
      }),
    ).toEqual({ kind: "lists", will: ["霧港"], maybe: ["讀不懂"] });
  });

  it("可繼續的確認窗都寫目前版本會被略過，不合格那扇不寫", () => {
    const note = "退回後，目前的 0.2.0 版會被標為略過";
    const invalid = rollbackDialogText({ kind: "invalid", message: PLATFORM_MISMATCH }, "0.2.0");
    expect(invalid).not.toContain(note);
    expect(invalid).toBe("無法回到這一版：平台不符");
    expect(rollbackDialogText({ kind: "scanFailed" }, "0.2.0")).toContain("無法預先判讀");
    expect(rollbackDialogText({ kind: "scanFailed" }, "0.2.0")).toContain(note);
    expect(rollbackDialogText({ kind: "none" }, "0.2.0")).toContain("沒有發現會變唯讀的桌");
    const lists = rollbackDialogText({ kind: "lists", will: ["霧港"], maybe: [] }, "0.2.0");
    expect(lists).toContain("會變唯讀的桌：\n霧港");
    expect(lists).not.toContain("可能唯讀的桌");
    expect(lists).toContain(note);
  });
});

describe("cannotReplace／updateChanged", () => {
  afterEach(() => setLang("zh-TW"));

  it("只認起首的後端代碼，不看文字", () => {
    expect(cannotReplace(CANNOT_REPLACE)).toBe(true);
    expect(cannotReplace(`Error: ${CANNOT_REPLACE}`)).toBe(true);
    expect(cannotReplace(SIGNATURE_INVALID)).toBe(false);
    expect(cannotReplace("無法自動替換")).toBe(false);
    expect(cannotReplace(`下載失敗：${CANNOT_REPLACE}`)).toBe(false);
    expect(updateChanged(UPDATE_CHANGED)).toBe(true);
    expect(updateChanged(CANNOT_REPLACE)).toBe(false);
  });

  it("切到俄文仍判得出來，顯示換成俄文", () => {
    setLang("ru");
    expect(cannotReplace(CANNOT_REPLACE)).toBe(true);
    expect(updateChanged(UPDATE_CHANGED)).toBe(true);
    const text = rollbackDialogText({ kind: "invalid", message: PLATFORM_MISMATCH }, "0.2.0");
    expect(text).toBe("Нельзя вернуться к этой версии: Не та платформа");
  });

  it("空的寫 0 KB，不到 1 KB 進位成 1 KB", () => {
    expect(formatBytes(0)).toBe("0 KB");
    expect(formatBytes(10)).toBe("1 KB");
    expect(formatBytes(5 * 1024)).toBe("5 KB");
  });
});
