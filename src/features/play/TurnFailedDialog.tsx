// 回合沒完成的攔截式彈窗：只有「關閉」。打字送出卻沒能自動收回時，玩家剛送出的原文放在這裡，
// 關掉前都選得到、複製得走（不另做複製鈕）。
import { t } from "../../i18n";
import { Dialog } from "../../shared/ui/Dialog";
import { AiErrorText } from "../../shared/ui/atoms";
import type { TurnFailure } from "./useChatController";

/** 彈窗還開著且帶玩家原文時，後續失敗不覆寫它——原文關窗前都要拿得到 */
export function nextTurnFailure(previous: TurnFailure | null, next: TurnFailure): TurnFailure {
  return previous?.draft !== undefined ? previous : next;
}

export function TurnFailedDialog({
  failure,
  transport,
  onClose,
}: {
  failure: TurnFailure;
  transport?: string;
  onClose: () => void;
}) {
  return (
    <Dialog
      title={t("turnFailedTitle")}
      size="s"
      className="turn-failed-dialog"
      onDismiss={onClose}
      end={
        <button type="button" className="btn btn-primary" data-autofocus="" onClick={onClose}>
          {t("closeBtn")}
        </button>
      }
    >
      <p role="alert">
        <AiErrorText text={failure.raw} transport={transport} />
      </p>
      {failure.draft !== undefined && (
        <label className="turn-failed-draft">
          {t("turnFailedDraftLabel")}
          <textarea readOnly rows={3} value={failure.draft} />
        </label>
      )}
    </Dialog>
  );
}
