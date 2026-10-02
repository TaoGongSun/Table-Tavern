import { t } from "../../i18n";
import { backendText } from "../../shared/ui/backend-text";
import { Dialog, SwapLabel } from "../../shared/ui/Dialog";
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
      {/* 執行中不給 Esc／遮罩：只能按取消，取消送出後等迴圈自己收尾 */}
      {progress && (
        <Dialog
          title={t("refactorBtn")}
          start={
            <button
              type="button"
              className="btn"
              disabled={progress.cancelling}
              onClick={cancelAiRefactor}
            >
              {t("refactorCancel")}
            </button>
          }
        >
          <p role="status">{progress.text}</p>
          {/* CLI 卡死／異常結束時後端會在串流尾巴插一行 ⚠ 代碼，顯示時才翻 */}
          {progress.tail && <pre className="refactor-stream-tail">{backendText(progress.tail)}</pre>}
        </Dialog>
      )}

      {modeAsk && (
        // 二選一（refactor-mode-split，2026-08-14 拍板文案）：一鍵照建議＋展開自己選兩層都有。
        // 取消＝整個不跑，反悔路是重按重構鈕重跑（原卡 PNG 留檔）。Esc＝取消、點遮罩不關。
        // 展開後「自己選」停用但占位、主鈕換字不變寬，按鈕都不位移
        <Dialog
          title={t("refactorModeTitle")}
          onDismiss={() => setModeAsk(null)}
          start={
            <>
              <button type="button" className="btn" onClick={() => setModeAsk(null)}>
                {t("refactorCancel")}
              </button>
              <button
                type="button"
                className="btn"
                disabled={modeAsk.expanded}
                onClick={() => setModeAsk({ ...modeAsk, expanded: true })}
              >
                {t("refactorModeChoose")}
              </button>
            </>
          }
          end={
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => {
                const { picked, ticket } = modeAsk;
                setModeAsk(null);
                void startRefactorRun(picked, ticket);
              }}
            >
              <SwapLabel
                labels={[t("refactorModeGoRecommended"), t("refactorModeGo")]}
                current={modeAsk.expanded ? 1 : 0}
              />
            </button>
          }
        >
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
        </Dialog>
      )}
    </>
  );
}
