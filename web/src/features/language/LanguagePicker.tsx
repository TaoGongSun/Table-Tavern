import { LANGUAGE_OPTIONS, type Lang } from "@desktop/i18n/languages";
import { t } from "../../i18n";

/** 頁首的語系選單（語系名照各語言自己的寫法，同桌面版設定頁）。locked：這時不能選的語系 */
export function LanguagePicker({
  lang,
  onChange,
  locked,
}: {
  lang: Lang;
  onChange: (lang: Lang) => void;
  locked: (lang: Lang) => boolean;
}) {
  return (
    <select
      className="lang-picker"
      aria-label={t("language")}
      value={lang}
      onChange={(event) => onChange(event.target.value as Lang)}
      data-testid="lang-picker"
    >
      {LANGUAGE_OPTIONS.map((option) => (
        <option key={option.value} value={option.value} disabled={locked(option.value)}>
          {option.label}
        </option>
      ))}
    </select>
  );
}
