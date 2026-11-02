import { LANGUAGE_OPTIONS, type Lang } from "@desktop/i18n/languages";
import { t } from "../../i18n";

/** 頁首的語系選單（語系名照各語言自己的寫法，同桌面版與網頁版） */
export function LanguagePicker({ lang, onChange }: { lang: Lang; onChange: (lang: Lang) => void }) {
  return (
    <select className="lang-picker" aria-label={t("language")} value={lang} onChange={(e) => onChange(e.target.value as Lang)}>
      {LANGUAGE_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
    </select>
  );
}
