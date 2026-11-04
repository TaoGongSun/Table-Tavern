// 「怎麼玩？」：連到網頁版的多人房間玩法說明（另開分頁）。朋友第一眼看到的暱稱頁與名單頁底部各一個。
import { t } from "../../i18n";

const ROOM_GUIDE_URL = "https://table-tavern.pages.dev/#guide/room";

export function GuideLink() {
  return (
    <a className="muted guide-link" href={ROOM_GUIDE_URL} target="_blank" rel="noopener noreferrer">
      {t("guideLink")}
    </a>
  );
}
