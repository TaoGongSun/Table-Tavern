import { t } from "../../i18n";
import { CARD_SITES } from "./card-sites";

/** 開始畫面的「去哪裡找角色卡」：五站連結＋一行 18 禁標示（D6）。 */
export function FindCards() {
  return (
    <article className="card-tile panel find-cards" data-testid="find-cards">
      <div className="card-tile-head">
        <h2>{t("findTitle")}</h2>
      </div>
      <p>{t("findBody")}</p>
      <p className="find-adult">{t("findAdult")}</p>
      <ul className="site-list">
        {CARD_SITES.map((site) => (
          <li key={site.id}>
            <a href={site.url} target="_blank" rel="noopener noreferrer">
              {site.name}
            </a>
            <span>{t(site.blurb)}</span>
          </li>
        ))}
      </ul>
    </article>
  );
}
