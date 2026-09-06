import { t } from "../../i18n";
import type { RefactorWorkflowController } from "./useRefactorWorkflow";

interface RefactorRunDialogsProps {
  refactor: RefactorWorkflowController;
}

export function RefactorRunDialogs({ refactor }: RefactorRunDialogsProps) {
  const {
    progress,
    modeAsk,
    setModeAsk,
    cancelAiRefactor,
    startRefactorRun,
  } = refactor;

  return (
    <>
      {progress && (
        <div className="modal-overlay">
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("refactorBtn")}>
            <h2>{t("refactorBtn")}</h2>
            <p role="status">{progress.text}</p>
            {progress.tail && <pre className="refactor-stream-tail">{progress.tail}</pre>}
            <div className="ai-gen-footer">
              <button type="button" disabled={progress.cancelling} onClick={cancelAiRefactor}>
                {t("refactorCancel")}
              </button>
            </div>
          </div>
        </div>
      )}

      {modeAsk && (
        // 二選一（refactor-mode-split，2026-08-14 拍板文案）：一鍵照建議＋展開自己選兩層都有。
        // 取消＝整個不跑，反悔路是重按重構鈕重跑（原卡 PNG 留檔）。
        <div className="modal-overlay">
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("refactorModeTitle")}>
            <h2>{t("refactorModeTitle")}</h2>
            {modeAsk.recommend !== null && (
              <p>
                {modeAsk.recommend === "interface"
                  ? t("refactorModeSuggestInterface", { evidence: modeAsk.evidence })
                  : t("refactorModeSuggestCharacters", { evidence: modeAsk.evidence })}
              </p>
            )}
            {modeAsk.expanded && (
              <div className="refactor-mode-options">
                {(["interface", "characters"] as const).map((option) => (
                  <label key={option} className="refactor-mode-option">
                    <input
                      type="radio"
                      name="refactor-mode"
                      checked={modeAsk.picked === option}
                      onChange={() => setModeAsk({ ...modeAsk, picked: option })}
                    />
                    <span>
                      <strong>
                        {option === "interface"
                          ? t("refactorModeOptInterface")
                          : t("refactorModeOptCharacters")}
                      </strong>
                      <br />
                      {option === "interface"
                        ? t("refactorModeOptInterfaceDesc")
                        : t("refactorModeOptCharactersDesc")}
                    </span>
                  </label>
                ))}
              </div>
            )}
            <div className="ai-gen-footer">
              <button type="button" onClick={() => setModeAsk(null)}>
                {t("refactorCancel")}
              </button>
              {!modeAsk.expanded && (
                <button type="button" onClick={() => setModeAsk({ ...modeAsk, expanded: true })}>
                  {t("refactorModeChoose")}
                </button>
              )}
              <button
                type="button"
                className="ai-gen-submit"
                onClick={() => {
                  const { picked, ticket } = modeAsk;
                  setModeAsk(null);
                  void startRefactorRun(picked, ticket);
                }}
              >
                {modeAsk.expanded ? t("refactorModeGo") : t("refactorModeGoRecommended")}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
