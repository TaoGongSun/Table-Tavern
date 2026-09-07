import { type FormEvent, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { t } from "../i18n";
import { RefactorResultDialog } from "../features/refactor/RefactorResultDialog";
import { RefactorRunDialogs } from "../features/refactor/RefactorRunDialogs";
import { WorldbookSection } from "../features/worldbook/WorldbookSection";
import { useRefactorWorkflow } from "../features/refactor/useRefactorWorkflow";
import { useWorldbookEditor } from "../features/worldbook/useWorldbookEditor";

// 世界書 v1：一份只進 GM 上下文的 world.md（NewPlan §7.0）
export function WorldEditor({
  world,
  worldName,
  onBack,
  leaveGuard,
  convertColor,
  onEntryConverted,
  onRefactorApplied,
}: {
  world: string;
  worldName: string;
  onBack: () => void;
  /** 側欄要離開世界設定時先問過這裡（未儲存確認與返回鈕同一條） */
  leaveGuard: { current: (() => Promise<boolean>) | null };
  convertColor: string;
  onEntryConverted: () => Promise<void>;
  /** AI 卡重構套用成功後：角色清單／卡片介面／桌面狀態都可能變了，交回 App 層重載 */
  onRefactorApplied: () => Promise<void>;
}) {
  const [text, setText] = useState<string | null>(null);
  const [savedText, setSavedText] = useState("");
  const [message, setMessage] = useState("");

  const worldbook = useWorldbookEditor({ world, convertColor, onEntryConverted });

  async function refreshAfterApply() {
    await worldbook.refreshWorldbook();
    await worldbook.refreshLedger();
    await worldbook.refreshCast();
    await onRefactorApplied();
  }

  const refactor = useRefactorWorkflow({
    world,
    worldName,
    entries: worldbook.entries,
    setStatusMessage: worldbook.setMessage,
    refreshAfterApply,
  });

  useEffect(() => {
    setMessage("");
    setText(null);
    invoke<string>("read_world_md", { worldId: world })
      .then((value) => {
        setText(value);
        setSavedText(value);
      })
      .catch((reason) => setMessage(String(reason)));
  }, [world]);

  if (text === null) return message ? <p role="alert">{message}</p> : null;

  const unsavedCount = (text !== savedText ? 1 : 0) + (worldbook.newEntryDirty ? 1 : 0);

  async function saveWorldSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    try {
      await invoke("write_world_md", { worldId: world, content: text });
      setSavedText(text ?? "");
      setMessage(t("saved"));
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function confirmLeave() {
    // 開著的既有條目照換編輯對象那套：先存起來再走，存不起來就別走。
    if (!(await worldbook.flushExistingDraftForLeave())) return false;
    if (unsavedCount === 0) return true;
    return await confirm(t("unsavedLeaveConfirm", { n: unsavedCount }), {
      title: t("unsavedLeaveTitle"),
      kind: "warning",
    });
  }
  // 側欄切走時走的是同一條確認；每次 render 掛上，閉包才拿得到最新的 unsavedCount。
  leaveGuard.current = confirmLeave;

  async function handleBack() {
    if (await confirmLeave()) onBack();
  }

  return (
    <>
      <form onSubmit={saveWorldSettings} className="settings-form">
        {/* 按鈕列放文字框上方：長文編輯時儲存／返回固定在最顯眼處（2026-07-24 使用者回饋） */}
        <div className="row">
          <button type="submit">{t("saveWorld")}</button>
          <button type="button" onClick={() => void handleBack()}>
            {t("backToNow")}
          </button>
          {message && <span>{message}</span>}
          {unsavedCount > 0 && (
            <span className="unsaved-hint" role="status">
              {t("unsavedChanges", { n: unsavedCount })}
            </span>
          )}
        </div>
        <textarea
          rows={6}
          aria-label={t("worldAria")}
          value={text}
          onChange={(event) => setText(event.currentTarget.value)}
        />
      </form>

      <WorldbookSection
        worldbook={worldbook}
        refactorRunning={refactor.progress !== null}
        refactorInputRef={refactor.inputRef}
        onRunRefactor={refactor.runAiRefactor}
        onPickRefactorOutcome={refactor.pickRefactorOutcome}
        onExportSavedRefactorOutcome={refactor.exportSavedRefactorOutcome}
      />

      <RefactorRunDialogs refactor={refactor} />
      <RefactorResultDialog refactor={refactor} entries={worldbook.entries} />
    </>
  );
}
