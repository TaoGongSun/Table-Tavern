// 精靈 ① 點子與偏好：兩個分頁「從點子生成｜用我的角色卡」。點子分頁第一眼只有一句話、題材、直接開始，
// 其餘偏好收在「更多偏好」；全部選填，什麼都不填也能生成（AI 自選題材）。卡分頁選檔交給匯入流程開 VN 桌。
import { useRef, useState } from "react";
import { t } from "../../../i18n";
import { SpriteSourceSelect } from "../art-flow/SpriteSourceSelect";
import { SwapLabel } from "../../../shared/ui/Dialog";
import type { TableGenesis } from "../../lobby/useTableGenesis";
import { CHOICES, ideaPlaceholder, LENGTHS, MOODS, VN_GENRES, type PreferenceDraft } from "./vn-preferences";

function Choice<T extends string>({
  label,
  options,
  value,
  disabled,
  onChange,
}: {
  label: string;
  options: readonly (readonly [T, Parameters<typeof t>[0]])[];
  value: T | null;
  disabled: boolean;
  onChange: (value: T | null) => void;
}) {
  return (
    <fieldset className="vn-pref-choice" disabled={disabled}>
      <legend>{label}</legend>
      {options.map(([id, key]) => (
        <button
          key={id}
          type="button"
          className={`gen-genre${value === id ? " gen-genre-selected" : ""}`}
          aria-pressed={value === id}
          onClick={() => onChange(value === id ? null : id)}
        >
          {t(key)}
        </button>
      ))}
    </fieldset>
  );
}

export function IdeaStep({
  genesis,
  draft,
  setDraft,
  onGenerate,
  onStart,
  spoilerLock,
  setSpoilerLock,
  onImportCard,
}: {
  genesis: TableGenesis;
  draft: PreferenceDraft;
  setDraft: (update: (draft: PreferenceDraft) => PreferenceDraft) => void;
  onGenerate: () => void;
  /** 不審大綱，串起生成、建桌、寫劇本，進標題畫面 */
  onStart: () => void;
  spoilerLock: boolean;
  setSpoilerLock: (locked: boolean) => void;
  onImportCard: (file: File, spoilerLock: boolean) => void;
}) {
  const [tab, setTab] = useState<"idea" | "card">("idea");
  const [more, setMore] = useState(false);
  const cardInput = useRef<HTMLInputElement>(null);
  const busy = genesis.busy !== null;
  const set = <K extends keyof PreferenceDraft>(key: K, value: PreferenceDraft[K]) =>
    setDraft((current) => ({ ...current, [key]: value }));
  return (
    <div className="vn-idea-step">
      <div className="vn-tabs" role="tablist">
        {(["idea", "card"] as const).map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            className={`vn-tab${tab === id ? " vn-tab-active" : ""}`}
            disabled={busy}
            onClick={() => setTab(id)}
          >
            {t(id === "idea" ? "vnTabIdea" : "vnTabCard")}
          </button>
        ))}
      </div>
      {tab === "card" ? (
        <div className="vn-card-drop">
          <p>{t("vnCardTabBody")}</p>
          <button type="button" className="btn btn-primary" onClick={() => cardInput.current?.click()}>
            {t("vnCardPick")}
          </button>
          <label className="inline">
            <input
              type="checkbox"
              checked={spoilerLock}
              onChange={(event) => setSpoilerLock(event.currentTarget.checked)}
            />
            {t("vnSpoilerCheckbox")}
          </label>
          <input
            ref={cardInput}
            type="file"
            accept=".png,.json,image/png,application/json"
            hidden
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              event.currentTarget.value = "";
              if (file) onImportCard(file, spoilerLock);
            }}
          />
        </div>
      ) : (
        <>
          <label className="vn-idea-label" htmlFor="vn-idea">
            {t("vnIdeaPrompt")}
          </label>
          <textarea
            id="vn-idea"
            data-autofocus=""
            rows={3}
            value={genesis.input}
            placeholder={ideaPlaceholder(draft)}
            disabled={busy}
            onChange={(event) => genesis.setInput(event.currentTarget.value)}
          />
          <div className="gen-genres">
            {VN_GENRES.map((key) => {
              const selected = draft.genres.includes(key);
              return (
                <button
                  key={key}
                  type="button"
                  className={`gen-genre${selected ? " gen-genre-selected" : ""}`}
                  aria-pressed={selected}
                  disabled={busy}
                  onClick={() =>
                    set("genres", selected ? draft.genres.filter((genre) => genre !== key) : [...draft.genres, key])
                  }
                >
                  {t(key)}
                </button>
              );
            })}
          </div>
          <button
            type="button"
            className="btn btn-ghost btn-sm vn-more-toggle"
            aria-expanded={more}
            onClick={() => setMore(!more)}
          >
            {t("vnMorePrefs")} {more ? "▾" : "▸"}
          </button>
          {more && (
            <div className="vn-more-prefs">
              <Choice label={t("vnMood")} options={MOODS} value={draft.mood} disabled={busy} onChange={(v) => set("mood", v)} />
              <Choice
                label={t("vnLength")}
                options={LENGTHS}
                value={draft.length}
                disabled={busy}
                onChange={(v) => set("length", v)}
              />
              <Choice
                label={t("vnChoices")}
                options={CHOICES}
                value={draft.branching}
                disabled={busy}
                onChange={(v) => set("branching", v)}
              />
              <label className="vn-pref-name">
                {t("vnYourName")}
                <input
                  value={draft.name}
                  placeholder={t("vnYourNamePlaceholder")}
                  disabled={busy}
                  onChange={(event) => set("name", event.currentTarget.value)}
                />
              </label>
            </div>
          )}
          <SpriteSourceSelect disabled={busy} />
          <p className="field-note">{t("vnArtAutoQuota")}</p>
          <div className="gen-generate-row">
            <button type="button" className="btn btn-primary" disabled={busy} onClick={onStart}>
              {t("vnStartNow")}
            </button>
            <button type="button" className="btn" disabled={busy} onClick={onGenerate}>
              <SwapLabel
                labels={[t(genesis.hasOutline ? "vnRewriteOutline" : "vnSeeOutline"), t("genGenerating")]}
                current={genesis.busy === "outline" ? 1 : 0}
              />
            </button>
            <small>{t("genQuotaNote")}</small>
          </div>
        </>
      )}
    </div>
  );
}
