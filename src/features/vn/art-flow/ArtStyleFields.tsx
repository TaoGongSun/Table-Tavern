// 畫風輸入框（精靈 ①、美術面板與精靈 ③ 共用）：沒填時只佔一行；「不想出現的東西」預設收合、有內容自動展開。
import { useEffect, useState, type ReactNode, type Ref } from "react";
import { t } from "../../../i18n";
import type { PlayerStyle } from "../../../shared/contracts/backend-contracts";
import "./art-style.css";

/** 與後端上限相同（data/vn_art.rs STYLE_CAP／AVOID_CAP） */
const STYLE_MAX = 600;
const AVOID_MAX = 300;

export function ArtStyleFields({
  value,
  onChange,
  onBlur,
  disabled = false,
  styleRef,
  folded = false,
  children,
}: {
  value: PlayerStyle;
  onChange: (next: PlayerStyle) => void;
  onBlur?: () => void;
  disabled?: boolean;
  styleRef?: Ref<HTMLTextAreaElement>;
  /** 整組收成一行、點開才填（精靈 ①：版面留給故事點子） */
  folded?: boolean;
  /** 欄位下方的狀態小字 */
  children?: ReactNode;
}) {
  const [open, setOpen] = useState(Boolean(value.avoid));
  // 收合的那一層：已有內容就先打開，之後照玩家自己開關
  const [unfolded, setUnfolded] = useState(Boolean(value.style || value.avoid));
  useEffect(() => {
    if (value.avoid) setOpen(true);
  }, [value.avoid]);
  const fields = (
    <>
      <label className="vn-art-style-field">
        {!folded && t("vnStyleLabel")}
        <textarea
          ref={styleRef}
          rows={value.style ? 2 : 1}
          maxLength={STYLE_MAX}
          value={value.style}
          placeholder={t("vnStylePlaceholder")}
          aria-label={t("vnStyleLabel")}
          disabled={disabled}
          onChange={(event) => onChange({ ...value, style: event.currentTarget.value })}
          onBlur={onBlur}
        />
      </label>
      <details open={open} onToggle={(event) => setOpen(event.currentTarget.open)}>
        <summary>{t("vnStyleAvoid")}</summary>
        <textarea
          rows={2}
          maxLength={AVOID_MAX}
          value={value.avoid}
          placeholder={t("vnStyleAvoidPlaceholder")}
          aria-label={t("vnStyleAvoid")}
          disabled={disabled}
          onChange={(event) => onChange({ ...value, avoid: event.currentTarget.value })}
          onBlur={onBlur}
        />
      </details>
    </>
  );
  return (
    <div className="vn-art-style">
      {folded ? (
        <details className="vn-art-style-fold" open={unfolded} onToggle={(event) => setUnfolded(event.currentTarget.open)}>
          <summary>{t("vnStyleLabel")}</summary>
          {fields}
        </details>
      ) : (
        fields
      )}
      {children}
    </div>
  );
}
