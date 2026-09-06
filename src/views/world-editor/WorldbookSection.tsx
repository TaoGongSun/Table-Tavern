import { Fragment, type RefObject, useEffect, useRef } from "react";
import { t } from "../../i18n";
import { useDragReorder } from "../../drag-reorder";
import { WorldbookEntryForm } from "./WorldbookEntryForm";
import type { WorldbookEditorController } from "./useWorldbookEditor";

interface WorldbookSectionProps {
  worldbook: WorldbookEditorController;
  refactorRunning: boolean;
  refactorInputRef: RefObject<HTMLInputElement | null>;
  onRunRefactor: () => void | Promise<void>;
  onPickRefactorOutcome: (file: File) => void | Promise<void>;
  onExportSavedRefactorOutcome: () => void | Promise<void>;
}

export function WorldbookSection({
  worldbook,
  refactorRunning,
  refactorInputRef,
  onRunRefactor,
  onPickRefactorOutcome,
  onExportSavedRefactorOutcome,
}: WorldbookSectionProps) {
  const {
    entries,
    ledger,
    characters,
    message,
    draft,
    draftOrigin,
    setDraft,
    refreshCharactersForVisibility,
    toggleLedgerEntry,
    addEntry,
    editEntry,
    closeDraft,
    saveEntry,
    deleteEntry,
    reorderEntries,
    dedupeWorldbook,
    exportWorldbook,
    convertEntryToCharacter,
  } = worldbook;

  const draftFormRef = useRef<HTMLFormElement>(null);
  const entryDrag = useDragReorder(
    entries,
    (entry) => String(entry.uid),
    (ordered) => void reorderEntries(ordered),
  );

  // 新增的空白表單排在清單底部，展開時可能在畫面外，捲到看得見
  // （不用 smooth：長清單的平滑捲動會被後續 render 打斷，停在半路）
  useEffect(() => {
    draftFormRef.current?.scrollIntoView({ block: "nearest" });
  }, [draftOrigin]);

  const entryForm = draft && (
    <WorldbookEntryForm
      draft={draft}
      characters={characters}
      formRef={draftFormRef}
      onSubmit={saveEntry}
      onCancel={closeDraft}
      onConvert={convertEntryToCharacter}
      onChange={setDraft}
      onRefreshCharacters={refreshCharactersForVisibility}
    />
  );

  return (
    <section className="worldbook-section" aria-labelledby="worldbook-title">
      <h3 id="worldbook-title">{t("worldbookTitle")}</h3>
      <div className="worldbook-actions">
        <button type="button" onClick={addEntry}>
          {t("worldbookAddEntry")}
        </button>
        <button type="button" onClick={() => void dedupeWorldbook()}>
          {t("worldbookDedupe")}
        </button>
        <button type="button" onClick={() => void exportWorldbook()}>
          {t("worldbookExport")}
        </button>
        <button
          type="button"
          className="ai-gen-btn"
          title={t("refactorBtnHint")}
          disabled={refactorRunning}
          onClick={() => void onRunRefactor()}
        >
          ✨ {t("refactorBtn")}
        </button>
        <button
          type="button"
          title={t("refactorImportBtnHint")}
          disabled={refactorRunning}
          onClick={() => refactorInputRef.current?.click()}
        >
          {t("refactorImportBtn")}
        </button>
        <button
          type="button"
          disabled={refactorRunning}
          onClick={() => void onExportSavedRefactorOutcome()}
        >
          {t("refactorExportSavedBtn")}
        </button>
        <input
          ref={refactorInputRef}
          type="file"
          accept=".json,application/json"
          hidden
          onChange={(event) => {
            const file = event.currentTarget.files?.[0];
            event.currentTarget.value = "";
            if (file) void onPickRefactorOutcome(file);
          }}
        />
      </div>
      {/* 操作回饋緊貼按鈕列：重構擋下訊息之類的結果放列表底部的話，條目多的桌要捲到底
          才看得到，點了像沒反應。 */}
      {message && <p role="status">{message}</p>}

      {/* 標準流程零必看：只有真的有東西被接管／跳過，或有記帳次數時才出現這塊。 */}
      {(ledger.entries.length > 0 ||
        ledger.rejected > 0 ||
        ledger.clamped > 0 ||
        ledger.errors > 0 ||
        ledger.jumps > 0) && (
        <details className="mechanism-ledger">
          <summary>{t("ledgerTitle")}</summary>
          {ledger.entries.length > 0 && (
            <div className="mechanism-ledger-list">
              {ledger.entries.map((entry) => (
                <div className="mechanism-ledger-row" key={entry.uid}>
                  <div className="mechanism-ledger-summary">
                    <strong>{entry.title}</strong>
                    <span className="worldbook-badge">
                      {entry.kind === "absorbed" ? t("ledgerAbsorbed") : t("ledgerSkipped")}
                    </span>
                    <span className="mechanism-ledger-detail">{entry.detail}</span>
                  </div>
                  {!entries.find((worldbookEntry) => worldbookEntry.uid === entry.uid)?.locked && (
                    <label className="mechanism-ledger-toggle">
                      <input
                        type="checkbox"
                        checked={entry.sent}
                        onChange={() => void toggleLedgerEntry(entry)}
                      />
                      {t("ledgerSendRaw")}
                    </label>
                  )}
                </div>
              ))}
            </div>
          )}
          {(ledger.rejected > 0 || ledger.clamped > 0 || ledger.errors > 0 || ledger.jumps > 0) && (
            <p className="mechanism-ledger-stats">
              {[
                ledger.rejected > 0 && t("ledgerStatsRejected", { n: ledger.rejected }),
                ledger.clamped > 0 && t("ledgerStatsClamped", { n: ledger.clamped }),
                ledger.errors > 0 && t("ledgerStatsErrors", { n: ledger.errors }),
                ledger.jumps > 0 && t("ledgerStatsJumps", { n: ledger.jumps }),
              ]
                .filter(Boolean)
                .join("　")}
            </p>
          )}
        </details>
      )}

      {entries.length === 0 ? (
        <p className="worldbook-empty">{t("worldbookEmpty")}</p>
      ) : (
        <div className="worldbook-list">
          {entryDrag.order.map((entry) =>
            draft && draft.uid === entry.uid ? (
              <Fragment key={entry.uid}>{entryForm}</Fragment>
            ) : (
              <div
                className={`worldbook-row${entry.disabled ? " worldbook-row-disabled" : ""}${
                  entryDrag.draggingKey === String(entry.uid) ? " row-dragging" : ""
                }`}
                key={entry.uid}
                title={t("dragToReorder")}
                {...entryDrag.rowProps(entry)}
              >
                <div className="worldbook-summary">
                  {(() => {
                    const head = (
                      <>
                        <strong>{entry.title || entry.uid}</strong>
                        <span>{entry.keys.join("、") || t("worldbookNoKeys")}</span>
                        <div className="worldbook-badges">
                          {entry.constant && <span className="worldbook-badge">{t("worldbookConstant")}</span>}
                          {/* 可見範圍＝資訊邊界：全 app 統一的虛線琥珀機密記號 */}
                          <span className="worldbook-badge worldbook-badge-visibility">
                            {entry.visibility.type === "gm"
                              ? t("worldbookVisibilityGm")
                              : entry.visibility.type === "public"
                                ? t("worldbookVisibilityPublic")
                                : t("worldbookCharacterCount", { n: entry.visibility.characters.length })}
                          </span>
                          {entry.disabled && <span className="worldbook-badge">{t("worldbookDisabled")}</span>}
                          {entry.locked && <span className="worldbook-badge">🔒 {t("worldbookLocked")}</span>}
                        </div>
                      </>
                    );
                    // 鎖定條目不能編輯，但說明文的家就在世界書：標題列可展開唯讀全文。
                    return entry.locked ? (
                      <details className="worldbook-locked-view">
                        <summary>{head}</summary>
                        <div className="worldbook-locked-content">{entry.content}</div>
                      </details>
                    ) : (
                      head
                    );
                  })()}
                </div>
                {!entry.locked && (
                  <div className="worldbook-row-actions">
                    <button type="button" onClick={() => editEntry(entry)}>
                      {t("editBtn")}
                    </button>
                    <button type="button" onClick={() => void deleteEntry(entry)}>
                      {t("worldbookDelete")}
                    </button>
                  </div>
                )}
              </div>
            ),
          )}
        </div>
      )}
      {draft && draft.uid === null && entryForm}
    </section>
  );
}
