import { useMemo, useState } from "react";
import { renderHostMarkdown } from "../../shared/ui/host-markdown";
import { t } from "../../i18n";
import type { GameSetup } from "../chat/useChat";
import { substituteParams } from "../sillytavern/substitute";
import { createChatVariables } from "../sillytavern/variables";
import type { CardRoute, PlayCard } from "./play-card";

const PREVIEW = 160;

/** 匯入後開玩前：挑開場白、確認匯入身分。一鍵「開始」用預設值，想改的再改。 */
export function CardImport({
  card,
  userName,
  onStart,
  onCancel,
}: {
  card: PlayCard;
  userName: string;
  onStart: (game: GameSetup) => void;
  onCancel: () => void;
}) {
  const [opening, setOpening] = useState(0);
  const [route, setRoute] = useState<CardRoute>(card.route);
  // 卡內 regex 腳本照 ST 問一次要不要允許（D21），預設照 ST 不允許
  const [regexAllowed, setRegexAllowed] = useState(card.regexAllowed);
  // 預覽只換名字之類的巨集，用一次性的變數表，不影響開桌後的狀態
  const previews = useMemo(() => {
    const context = { card: card.text, userName, chat: [], variables: createChatVariables(), chatId: "preview" };
    return card.openings.map((text) => {
      const filled = substituteParams(text, context).replace(/\s+/g, " ").trim();
      return filled.length > PREVIEW ? `${filled.slice(0, PREVIEW)}…` : filled;
    });
  }, [card, userName]);
  // 兩條路都收的卡才讓玩家選；桌面版會拒收的那條不列
  const askRoute = card.view.route.decision === "ask" && card.view.validity.worldbook === null;
  // 卡片 metadata 一律走安全渲染（計畫 2.4）
  const notes = useMemo(() => {
    const raw = card.text.creator_notes.trim();
    return raw ? renderHostMarkdown(raw.length > 600 ? `${raw.slice(0, 600)}…` : raw) : "";
  }, [card]);

  return (
    <section className="picker" data-testid="card-import">
      <h1>{card.text.name}</h1>
      <div className="import-actions">
        <button type="button" className="ghost" onClick={onCancel}>
          {t("importCancel")}
        </button>
        <button
          type="button"
          className="primary"
          onClick={() => onStart({ card: { ...card, route, regexAllowed }, userName, openingIndex: previews.length ? opening : null })}
        >
          {t("importStart")}
        </button>
      </div>
      {notes && <div className="import-notes" dangerouslySetInnerHTML={{ __html: notes }} />}

      <fieldset className="panel import-openings">
        <legend>{t("importOpening")}</legend>
        {previews.length === 0 && <p className="chat-note">{t("importNoOpening")}</p>}
        {previews.map((preview, index) => (
          <label key={index} className={`opening-option${index === opening ? " opening-chosen" : ""}`}>
            <input type="radio" name="opening" checked={index === opening} onChange={() => setOpening(index)} />
            <span>{preview}</span>
          </label>
        ))}
      </fieldset>

      {card.regexScripts.length > 0 && (
        <fieldset className="panel import-regex">
          <legend>{t("importRegex", { count: card.regexScripts.length })}</legend>
          <label className="opening-option">
            <input
              type="checkbox"
              data-testid="import-regex-allow"
              checked={regexAllowed}
              onChange={(event) => setRegexAllowed(event.target.checked)}
            />
            <span>{t("importRegexAllow")}</span>
          </label>
          <p className="chat-note">{t("importRegexHint")}</p>
        </fieldset>
      )}

      {askRoute && (
        <details className="import-advanced">
          <summary>
            {t("importRoute")}：{t(route === "character" ? "importRouteCharacter" : "importRouteWorldbook")}
          </summary>
          <p className="chat-note">{t("importRouteHint")}</p>
          {(["character", "worldbook"] as const).map((value) => (
            <label key={value} className="opening-option">
              <input type="radio" name="route" checked={route === value} onChange={() => setRoute(value)} />
              <span>{t(value === "character" ? "importRouteCharacter" : "importRouteWorldbook")}</span>
            </label>
          ))}
        </details>
      )}
    </section>
  );
}
