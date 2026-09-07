import { t, type MsgKey } from "../../i18n";
import {
  defaultRefactorSelection,
  refactorSummaryCounts,
  setPlayerIndex,
  sourceEntryTitle,
  sourceEntryTitles,
  toggleIndex,
  unselectCharacter,
} from "./refactor-review";
import type { WorldbookEntry } from "../../shared/contracts/backend-contracts";
import type { RefactorWorkflowController } from "./useRefactorWorkflow";

// 淘汰理由 rule／稽核 kind 都是後端固定枚舉，本地寫死對照 i18n 鍵；查不到就退第一種，不讓畫面空白。
const REFACTOR_DROPPED_RULE_KEYS: Record<number, MsgKey> = {
  1: "refactorDroppedRule1",
  2: "refactorDroppedRule2",
  3: "refactorDroppedRule3",
  4: "refactorDroppedRule4",
  5: "refactorDroppedRule5",
};
const REFACTOR_AUDIT_KIND_KEYS: Record<string, MsgKey> = {
  coverage: "refactorAuditKindCoverage",
  mechanism: "refactorAuditKindMechanism",
  split: "refactorAuditKindSplit",
  drop_rule: "refactorAuditKindDropRule",
  excused: "refactorAuditKindExcused",
};

interface RefactorResultDialogProps {
  refactor: RefactorWorkflowController;
  entries: WorldbookEntry[];
}

export function RefactorResultDialog({ refactor, entries }: RefactorResultDialogProps) {
  const {
    outcome,
    selection,
    detail,
    cancelled,
    failures,
    busy,
    setSelection,
    setDetail,
    closeRefactor,
    restoreDroppedItem,
    applyRefactor,
    exportRefactorOutcome,
  } = refactor;

  if (!outcome || !selection) return null;

  // 結果卡摘要行只列有產物的區：「拆出 N 個角色」「介面」「收編 N 條規則」以「・」串接
  const counts = refactorSummaryCounts(outcome);
  const summaryParts = [
    counts.characters > 0 && t("refactorSummaryCharacters", { n: counts.characters }),
    counts.hasInterface && t("refactorSummaryInterface"),
    counts.entries > 0 && t("refactorSummaryEntries", { n: counts.entries }),
    counts.mechanisms > 0 && t("refactorSummaryMechanisms", { n: counts.mechanisms }),
  ].filter((part): part is string => Boolean(part));

  return (
    // 點視窗外不關閉（2026-08-12 拍板）：誤觸一下整份重構結果就丟了，關閉只走「不要」鍵
    <div className="modal-overlay">
      <div className="modal" role="dialog" aria-modal="true" aria-label={t("refactorResultTitle")}>
        <h2>
          {cancelled
            ? t("refactorResultCancelledTitle")
            : failures.length > 0
              ? t("refactorResultPartialTitle")
              : t("refactorResultTitle")}
        </h2>
        {cancelled && (
          <p className="usage-bad" role="alert">
            {t("refactorCancelledNotice")}
          </p>
        )}
        {failures.length > 0 && (
          <p className="usage-bad" role="alert">
            {t("refactorPartialFailed", {
              n: failures.length,
              names: failures.map((failure) => failure.name).join("、"),
            })}
            {[...new Set(failures.map((failure) => failure.reason).filter(Boolean))].map((reason) => (
              <span key={reason} className="refactor-fail-reason">
                {t("refactorFailReason", { reason })}
              </span>
            ))}
          </p>
        )}
        {!detail ? (
          <>
            {summaryParts.length > 0 && <p>{summaryParts.join("・")}</p>}
            <div className="ai-gen-footer">
              {/* 取消造成的半成品：主按鈕換成「不要」，套用降級成次要鈕（2026-08-14 拍板）。 */}
              <button
                type="button"
                className={cancelled ? "ai-gen-submit" : undefined}
                disabled={busy}
                onClick={closeRefactor}
              >
                {t("refactorDismiss")}
              </button>
              <button type="button" disabled={busy} onClick={() => void exportRefactorOutcome()}>
                {t("refactorExportBtn")}
              </button>
              <button type="button" disabled={busy} onClick={() => setDetail(true)}>
                {t("refactorExpand")}
              </button>
              <button
                type="button"
                className={cancelled ? undefined : "ai-gen-submit"}
                disabled={busy}
                onClick={() => void applyRefactor(defaultRefactorSelection(outcome))}
              >
                {t("refactorApplyAll")}
              </button>
            </div>
          </>
        ) : (
          <>
            {outcome.characters.length > 0 && (
              <section>
                <h3>{t("refactorSectionCharacters")}</h3>
                <div className="mechanism-ledger-list">
                  {outcome.characters.map((character, index) => (
                    <div className="mechanism-ledger-row" key={index}>
                      <details>
                        <summary>
                          <label className="inline" onClick={(event) => event.stopPropagation()}>
                            <input
                              type="checkbox"
                              checked={selection.character_indices.includes(index)}
                              onClick={(event) => event.stopPropagation()}
                              onChange={(event) => {
                                const checked = event.currentTarget.checked;
                                setSelection(
                                  (current) =>
                                    current &&
                                    (checked
                                      ? {
                                          ...current,
                                          character_indices: toggleIndex(
                                            current.character_indices,
                                            index,
                                            true,
                                          ),
                                        }
                                      : unselectCharacter(current, index)),
                                );
                              }}
                            />
                            {character.emoji} {character.name}
                          </label>
                        </summary>
                        <span className="refactor-source">
                          {t("refactorSourceLabel", {
                            titles: sourceEntryTitles(entries, character.source_uids),
                          })}
                        </span>
                        <p>{t("refactorCharPublic")}</p>
                        <div style={{ whiteSpace: "pre-wrap" }}>{character.public_md}</div>
                        <p>{t("refactorCharPrivate")}</p>
                        <div style={{ whiteSpace: "pre-wrap" }}>{character.private_md}</div>
                      </details>
                      {/* 玩家卡只問 AI 認定是 {{user}} 的那一位：多數卡都預設好玩家是誰，
                          讓任意角色都能被選成玩家卡不符合卡的設計。 */}
                      {character.suspected_player && (
                        <label className="mechanism-ledger-toggle">
                          <input
                            type="checkbox"
                            checked={selection.player_index === index}
                            onChange={(event) =>
                              setSelection(
                                (current) =>
                                  current &&
                                  setPlayerIndex(current, event.currentTarget.checked ? index : null),
                              )
                            }
                          />
                          {t("refactorPlayerCheckLabel")}
                        </label>
                      )}
                    </div>
                  ))}
                </div>
              </section>
            )}
            {outcome.entries.length > 0 && (
              <section>
                <h3>{t("refactorSectionEntries")}</h3>
                <div className="mechanism-ledger-list">
                  {outcome.entries.map((entry, index) => (
                    <div className="mechanism-ledger-row" key={index}>
                      <details>
                        <summary>
                          <label className="inline" onClick={(event) => event.stopPropagation()}>
                            <input
                              type="checkbox"
                              checked={selection.entry_indices.includes(index)}
                              onClick={(event) => event.stopPropagation()}
                              onChange={(event) => {
                                const checked = event.currentTarget.checked;
                                setSelection(
                                  (current) =>
                                    current && {
                                      ...current,
                                      entry_indices: toggleIndex(
                                        current.entry_indices,
                                        index,
                                        checked,
                                      ),
                                    },
                                );
                              }}
                            />
                            {entry.title}
                          </label>
                          <span className="worldbook-badge">
                            {entry.kind === "setting"
                              ? t("refactorEntryKindSetting")
                              : t("refactorEntryKindMechanism")}
                          </span>
                          {entry.kind === "mechanism" &&
                            (Object.keys(entry.rules ?? {}).length > 0 ||
                              (entry.triggers?.length ?? 0) > 0) && (
                              <span className="worldbook-badge">🔒 {t("worldbookLocked")}</span>
                            )}
                        </summary>
                        <span className="refactor-source">
                          {t("refactorSourceLabel", {
                            titles: sourceEntryTitles(entries, entry.source_uids),
                          })}
                        </span>
                        <div style={{ whiteSpace: "pre-wrap" }}>{entry.content}</div>
                      </details>
                    </div>
                  ))}
                </div>
              </section>
            )}
            {outcome.interface && (
              <section>
                <h3>{t("refactorSectionInterface")}</h3>
                <div className="mechanism-ledger-list">
                  <details>
                    <summary>
                      <label className="inline" onClick={(event) => event.stopPropagation()}>
                        <input
                          type="checkbox"
                          checked={selection.apply_interface}
                          onClick={(event) => event.stopPropagation()}
                          onChange={(event) => {
                            const checked = event.currentTarget.checked;
                            setSelection(
                              (current) => current && { ...current, apply_interface: checked },
                            );
                          }}
                        />
                        {t("refactorSummaryInterface")}
                      </label>
                    </summary>
                    <span className="refactor-source">
                      {t("refactorSourceLabel", {
                        titles: sourceEntryTitles(entries, outcome.interface.source_uids),
                      })}
                    </span>
                    <div>
                      {t("refactorInterfaceFields", {
                        names:
                          typeof outcome.interface.state_fields === "object" &&
                          outcome.interface.state_fields !== null &&
                          !Array.isArray(outcome.interface.state_fields)
                            ? Object.keys(outcome.interface.state_fields).join("、")
                            : "",
                      })}
                    </div>
                  </details>
                </div>
              </section>
            )}
            {outcome.mechanisms.length > 0 && (
              <section>
                <h3>{t("refactorSectionMechanisms")}</h3>
                <div className="mechanism-ledger-list">
                  {outcome.mechanisms.map((mechanism, index) => (
                    <label className="inline" key={index}>
                      <input
                        type="checkbox"
                        checked={selection.mechanism_indices.includes(index)}
                        onChange={(event) => {
                          const checked = event.currentTarget.checked;
                          setSelection(
                            (current) =>
                              current && {
                                ...current,
                                mechanism_indices: toggleIndex(
                                  current.mechanism_indices,
                                  index,
                                  checked,
                                ),
                              },
                          );
                        }}
                      />
                      {sourceEntryTitle(entries, mechanism.source_uid)}
                    </label>
                  ))}
                </div>
              </section>
            )}
            {/* 已淘汰：判官整條／半條丟棄的內容快照，預設收起——玩家想確認才展開，救回來就是
                普通世界書條目，走下面既有的套用路徑，沒有新的後端行為。 */}
            {outcome.dropped.length > 0 && (
              <details className="mechanism-ledger">
                <summary>{t("refactorDroppedSection", { n: outcome.dropped.length })}</summary>
                <div className="mechanism-ledger-list">
                  {outcome.dropped.map((item, index) => (
                    <div className="mechanism-ledger-row" key={index}>
                      <details>
                        <summary>
                          {item.title}{" "}
                          <span className="worldbook-badge">
                            {t(REFACTOR_DROPPED_RULE_KEYS[item.rule] ?? "refactorDroppedRule1")}
                          </span>
                        </summary>
                        <div style={{ whiteSpace: "pre-wrap" }}>{item.content}</div>
                      </details>
                      <button type="button" onClick={() => restoreDroppedItem(index)}>
                        {t("refactorDroppedRestore")}
                      </button>
                    </div>
                  ))}
                </div>
              </details>
            )}
            {/* 未接管機制：純資訊，原文已經照搬進 GM 規則條目——不會遺失，只是還沒有系統畫面。 */}
            {outcome.unabsorbed.length > 0 && (
              <section>
                <h3>{t("refactorUnabsorbedSection", { n: outcome.unabsorbed.length })}</h3>
                <p className="mechanism-ledger-detail">{t("refactorUnabsorbedHint")}</p>
                <div className="mechanism-ledger-list">
                  {outcome.unabsorbed.map((item, index) => (
                    <div className="mechanism-ledger-row" key={index}>
                      <div className="mechanism-ledger-summary">
                        <strong>{item.title}</strong>
                        <span className="mechanism-ledger-detail">{item.note}</span>
                        <span className="refactor-source">{item.span || item.uid}</span>
                      </div>
                    </div>
                  ))}
                </div>
              </section>
            )}
            {/* 稽核：機械檢查抓到的紅字，純資訊不影響套用——detail 是後端已經寫好的繁中一句。 */}
            {outcome.audit.length > 0 && (
              <section>
                <h3>{t("refactorAuditSection")}</h3>
                <div className="mechanism-ledger-list">
                  {outcome.audit.map((item, index) => (
                    <div className="mechanism-ledger-row" key={index}>
                      <div className="mechanism-ledger-summary">
                        <span className="worldbook-badge">
                          {t(REFACTOR_AUDIT_KIND_KEYS[item.kind] ?? "refactorAuditKindCoverage")}
                        </span>
                        <span className="refactor-source">{item.span || item.uid}</span>
                        <span
                          className={
                            item.kind === "excused" ? "mechanism-ledger-detail" : "usage-bad"
                          }
                        >
                          {item.detail}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </section>
            )}
            <div className="ai-gen-footer">
              <button type="button" disabled={busy} onClick={() => setDetail(false)}>
                {t("settingsBack")}
              </button>
              <button
                type="button"
                className="ai-gen-submit"
                disabled={busy}
                onClick={() => void applyRefactor(selection)}
              >
                {t("refactorApplyBtn")}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
