// 確認框：照編卡畫面的離開確認（全畫面遮罩、Esc 取消、預設焦點在取消），按鈕由呼叫方給（第一顆是主要動作）。
import type { ReactNode } from "react";
import { t } from "../../i18n";

export interface ConfirmAction {
  label: string;
  onClick: () => void;
  danger?: boolean;
}

export function Confirm({
  title,
  children,
  actions,
  onCancel,
  testId,
}: {
  title: string;
  children?: ReactNode;
  actions: ConfirmAction[];
  onCancel: () => void;
  testId?: string;
}) {
  return (
    <div
      className="leave-confirm"
      role="alertdialog"
      aria-label={title}
      data-testid={testId}
      onKeyDown={(e) => e.key === "Escape" && onCancel()}
    >
      <div className="leave-box">
        <h3>{title}</h3>
        {children}
        <div className="confirm-actions">
          {actions.map((a) => (
            <button
              key={a.label}
              type="button"
              className={a.danger ? "danger" : undefined}
              onClick={a.onClick}
            >
              {a.label}
            </button>
          ))}
          <button type="button" className="ghost" autoFocus onClick={onCancel}>
            {t("cancel")}
          </button>
        </div>
      </div>
    </div>
  );
}
