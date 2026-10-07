import { useRef, useState } from "react";
import { t, type MsgKey } from "../../i18n";
import type { GameSetup } from "../chat/useChat";
import { DesktopOnly } from "../funnel/DesktopOnly";
import type { ReleaseInfo } from "../funnel/releases";
import { FindCards } from "../find-cards/FindCards";
import type { SaveStore } from "../saves/save-store";
import { SavesPanel } from "../saves/SavesPanel";
import { substituteParams } from "../sillytavern/substitute";
import { createChatVariables } from "../sillytavern/variables";
import { CardImport } from "./CardImport";
import { importCardFile, type ImportErrorCode, type PlayCard } from "./play-card";
import { SAMPLE_PLAY_CARD } from "./sample-card";
import { cleanUserName, loadUserName, saveUserName } from "./user-name";

export function CardPicker({
  onStart,
  saves,
  persisted,
  release,
}: {
  onStart: (game: GameSetup) => void;
  saves: SaveStore | null;
  persisted: boolean | null;
  release: ReleaseInfo;
}) {
  const [userName, setUserName] = useState(() => loadUserName(t("defaultUserName")));
  const [imported, setImported] = useState<PlayCard | null>(null);
  const [error, setError] = useState<ImportErrorCode | null>(null);
  const [reading, setReading] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);
  const name = cleanUserName(userName) || t("defaultUserName");
  const sample = SAMPLE_PLAY_CARD;
  const sampleBlurb = substituteParams(sample.text.description, {
    card: sample.text,
    userName: name,
    chat: [],
    variables: createChatVariables(),
    chatId: "preview",
  });

  const start = (game: GameSetup) => {
    saveUserName(name);
    onStart(game);
  };

  const readFile = async (file: File) => {
    setError(null);
    setReading(true);
    try {
      const result = await importCardFile(file);
      if (result.ok) setImported(result.card);
      else setError(result.error);
    } catch {
      setError("card_json_invalid");
    } finally {
      setReading(false);
      if (fileRef.current) fileRef.current.value = "";
    }
  };

  if (imported) {
    return <CardImport card={imported} userName={name} onStart={start} onCancel={() => setImported(null)} />;
  }

  return (
    <section className="picker">
      <h1>{t("pickTitle")}</h1>
      <label className="user-name">
        <span>{t("pickUserName")}</span>
        <input value={userName} maxLength={60} onChange={(event) => setUserName(event.target.value)} />
      </label>

      <SavesPanel saves={saves} persisted={persisted} release={release} onContinue={onStart} />

      <article className="card-tile panel">
        <div className="card-tile-head">
          <h2>{sample.text.name}</h2>
          <span className="tag">{t("pickSampleTag")}</span>
        </div>
        <p>{sampleBlurb}</p>
        <button type="button" className="primary" onClick={() => start({ card: sample, userName: name, openingIndex: 0 })}>
          {t("pickStart")}
        </button>
      </article>

      <article className="card-tile panel">
        <div className="card-tile-head">
          <h2>{t("importTitle")}</h2>
        </div>
        <p>{t("importBody")}</p>
        {error && (
          <p className="chat-error" role="alert" data-testid="import-error">
            {t(`importErr_${error}` as MsgKey)}
            {error === "standalone_book" && <DesktopOnly />}
          </p>
        )}
        <input
          ref={fileRef}
          type="file"
          accept=".png,.json,image/png,application/json"
          hidden
          data-testid="card-file"
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (file) void readFile(file);
          }}
        />
        <button type="button" className="primary" disabled={reading} onClick={() => fileRef.current?.click()}>
          {reading ? t("importReading") : t("importChoose")}
        </button>
      </article>

      <FindCards />
    </section>
  );
}
