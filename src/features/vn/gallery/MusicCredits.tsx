// 美術與音樂面板的音樂頁：每筆曲名、作者、授權、來源。不在這裡播放。
import { openUrl } from "@tauri-apps/plugin-opener";
import { t } from "../../../i18n";
import type { AudioEntry } from "../../../shared/contracts/backend-contracts";
import { licenseLabel, useCreditEntries } from "../audio/credits";

function CreditSection({ title, entries }: { title: string; entries: AudioEntry[] }) {
  if (entries.length === 0) return null;
  return (
    <section className="vn-credits-section">
      <h3>{title}</h3>
      <ul className="vn-credits-list">
        {entries.map((entry) => (
          <li key={`${entry.kind}:${entry.key}`} className="vn-gallery-row vn-credits-row">
            <span className="vn-credits-title">{entry.title}</span>
            <span>{entry.author}</span>
            <span>{licenseLabel(entry.license)}</span>
            {entry.source_url.startsWith("https://") && (
              <button type="button" className="link" onClick={() => void openUrl(entry.source_url)}>
                {t("vnCreditsSource")}
              </button>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

export function MusicCredits({ worldId }: { worldId: string }) {
  const entries = useCreditEntries(worldId);
  const bgm = entries.filter((entry) => entry.kind === "bgm");
  const sfx = entries.filter((entry) => entry.kind === "sfx");
  if (bgm.length === 0 && sfx.length === 0) return <p className="vn-chapter-note">{t("vnCreditsEmpty")}</p>;
  return (
    <div className="vn-credits">
      <CreditSection title={t("vnCreditsMusic")} entries={bgm} />
      <CreditSection title={t("vnCreditsSfx")} entries={sfx} />
    </div>
  );
}
