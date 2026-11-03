// 選項面板：2–4 鈕＋「換一組」；換一組失敗的訊息顯示在下方，原組照留。
import { t } from "../../i18n";

export function ChoicePanel({
  options,
  disabled,
  error,
  onChoose,
  onReroll,
}: {
  options: string[];
  disabled: boolean;
  error: string | null;
  onChoose: (option: string) => void;
  onReroll: () => void;
}) {
  return (
    <div className="vn-choices">
      {options.map((option, index) => (
        <button
          key={`${index}:${option}`}
          type="button"
          className="btn vn-choice"
          disabled={disabled}
          onClick={() => onChoose(option)}
        >
          {option}
        </button>
      ))}
      <button type="button" className="btn btn-ghost" disabled={disabled} onClick={onReroll}>
        {t("vnReroll")}
      </button>
      {error && (
        <p className="vn-choice-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
