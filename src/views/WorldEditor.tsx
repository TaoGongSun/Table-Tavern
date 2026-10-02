import { type FormEvent, useEffect, useId, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { t } from "../i18n";
import { RefactorResultDialog } from "../features/refactor/RefactorResultDialog";
import { RefactorRunDialogs } from "../features/refactor/RefactorRunDialogs";
import { WorldbookSection } from "../features/worldbook/WorldbookSection";
import { useRefactorWorkflow } from "../features/refactor/useRefactorWorkflow";
import { useWorldbookEditor } from "../features/worldbook/useWorldbookEditor";
import { EditPage } from "./EditPage";

// 世界書 v1：一份只進 GM 上下文的 world.md（NewPlan §7.0）
export function WorldEditor({
  title,
  world,
  worldName,
  onBack,
  leaveGuard,
  convertColor,
  onEntryConverted,
  onRefactorApplied,
}: {
  title: string;
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
  // 頂列儲存鈕在表單外，用 form 屬性只送 world.md 這張表單
  const formId = useId();

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
    // 換桌後才回來的舊請求丟掉，不拿上一桌的 world.md 蓋這一桌
    let stale = false;
    invoke<string>("read_world_md", { worldId: world })
      .then((value) => {
        if (stale) return;
        setText(value);
        setSavedText(value);
      })
      .catch((reason) => {
        if (!stale) setMessage(String(reason));
      });
    return () => {
      stale = true;
    };
  }, [world]);

  // 已儲存之外的訊息都是失敗
  const messageIsError = message !== "" && message !== t("saved");

  // 載入中／載入失敗也畫頁框：儲存停用占位、返回可用；還沒東西可改，離開不必問
  if (text === null) {
    leaveGuard.current = async () => true;
    return (
      <EditPage title={title} onBack={onBack} message={message} messageIsError={messageIsError} />
    );
  }

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
      okLabel: t("unsavedLeaveOk"),
      cancelLabel: t("unsavedLeaveCancel"),
    });
  }
  // 側欄切走時走的是同一條確認；每次 render 掛上，閉包才拿得到最新的 unsavedCount。
  leaveGuard.current = confirmLeave;

  async function handleBack() {
    if (await confirmLeave()) onBack();
  }

  return (
    <EditPage
      title={title}
      onBack={() => void handleBack()}
      formId={formId}
      unsavedCount={unsavedCount}
      message={message}
      messageIsError={messageIsError}
    >
      {/* world form 只包這一格：世界書區有自己的條目表單，不能巢狀；頂列儲存只寫 world.md */}
      <form id={formId} onSubmit={saveWorldSettings} className="settings-form">
        <label>
          {t("worldSummary")}
          <textarea
            rows={6}
            value={text}
            onChange={(event) => setText(event.currentTarget.value)}
          />
        </label>
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
    </EditPage>
  );
}
