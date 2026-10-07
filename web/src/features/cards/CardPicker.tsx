import { t } from "../../i18n";
import { SAMPLE_CARD, type CharacterCardData } from "./sample-card";

const plain = (text: string, name: string) => text.replace(/\{\{char\}\}/gi, name);

export function CardPicker({ onPick }: { onPick: (card: CharacterCardData) => void }) {
  const card = SAMPLE_CARD.data;
  return (
    <section className="picker">
      <h1>{t("pickTitle")}</h1>
      <article className="card-tile panel">
        <div className="card-tile-head">
          <h2>{card.name}</h2>
          <span className="tag">{t("pickSampleTag")}</span>
        </div>
        <p>{plain(card.description, card.name)}</p>
        <button type="button" className="primary" onClick={() => onPick(card)}>
          {t("pickStart")}
        </button>
      </article>
    </section>
  );
}
