// 手機名單頁底部的「顯示」區：房號、語系、劇情字級（一次性設定，不常駐在劇情旁邊）。
import type { Lang } from "@desktop/i18n/languages";
import { t, type MsgKey } from "../../i18n";
import { LanguagePicker } from "../language/LanguagePicker";
import { STORY_SIZES, type StorySize } from "./story-size";

export function DisplaySettings({
  code,
  lang,
  onLang,
  size,
  onSize,
}: {
  code: string;
  lang: Lang;
  onLang: (lang: Lang) => void;
  size: StorySize;
  onSize: (size: StorySize) => void;
}) {
  return (
    <section className="display-settings" data-testid="display-settings">
      <h3>{t("displayTitle")}</h3>
      <div className="row">
        <span className="grow muted">{t("roomCode", { code })}</span>
        <LanguagePicker lang={lang} onChange={onLang} />
      </div>
      <div className="row" role="group" aria-label={t("storySize")}>
        <span className="grow">{t("storySize")}</span>
        <span className="size-seg">
          {(Object.keys(STORY_SIZES) as StorySize[]).map((k) => (
            <button
              key={k}
              type="button"
              className={k === size ? "on" : undefined}
              aria-pressed={k === size}
              data-testid={`story-size-${k}`}
              onClick={() => onSize(k)}
            >
              {t(`storySize_${k}` as MsgKey)}
            </button>
          ))}
        </span>
      </div>
    </section>
  );
}
