import type { FormEventHandler, RefObject } from "react";
import type { CharacterMeta } from "../characters/card-model";
import { t } from "../../i18n";
import type { WorldbookDraft } from "./worldbook-model";

interface WorldbookEntryFormProps {
  draft: WorldbookDraft;
  characters: CharacterMeta[];
  formRef: RefObject<HTMLFormElement | null>;
  onSubmit: FormEventHandler<HTMLFormElement>;
  onCancel: () => void | Promise<void>;
  onConvert: () => void | Promise<void>;
  onChange: (draft: WorldbookDraft) => void;
  onRefreshCharacters: () => void;
}

// 條目表單就地展開：編輯取代原本那一列、新增排在清單底部（2026-07-30 使用者回饋——
// 表單固定在頂端時，點下方條目的編輯完全看不出反應）。按鈕照全 app 慣例置頂。
export function WorldbookEntryForm({
  draft,
  characters,
  formRef,
  onSubmit,
  onCancel,
  onConvert,
  onChange,
  onRefreshCharacters,
}: WorldbookEntryFormProps) {
  return (
    <form ref={formRef} className="settings-form worldbook-form" onSubmit={onSubmit}>
      {/* 三顆都是次鈕：頁面的主鈕已是頂列儲存與新增條目。單行不換行，擠不下就截字 */}
      <div className="worldbook-form-actions">
        <button type="submit" className="btn btn-shrink" title={t("worldbookSaveEntry")}>
          <span className="btn-label">{t("worldbookSaveEntry")}</span>
        </button>
        <button
          type="button"
          className="btn btn-shrink"
          title={t("worldbookCancel")}
          onClick={() => void onCancel()}
        >
          <span className="btn-label">{t("worldbookCancel")}</span>
        </button>
        {draft.uid !== null && (
          <button
            type="button"
            className="btn btn-shrink"
            title={t("convertEntryToCard")}
            onClick={() => void onConvert()}
          >
            <span className="btn-label">{t("convertEntryToCard")}</span>
          </button>
        )}
      </div>
      <label>
        {t("worldbookEntryTitle")}
        <input
          value={draft.title}
          onChange={(event) => onChange({ ...draft, title: event.currentTarget.value })}
        />
      </label>
      <label>
        {t("worldbookKeys")}
        <input
          value={draft.keys}
          placeholder={t("worldbookKeysHint")}
          onChange={(event) => onChange({ ...draft, keys: event.currentTarget.value })}
        />
      </label>
      <label>
        {t("worldbookContent")}
        <textarea
          rows={7}
          value={draft.content}
          onChange={(event) => onChange({ ...draft, content: event.currentTarget.value })}
        />
      </label>
      <label className="inline">
        <input
          type="checkbox"
          checked={draft.constant}
          onChange={(event) => onChange({ ...draft, constant: event.currentTarget.checked })}
        />
        {t("worldbookConstantLabel")}
      </label>
      <label className="inline">
        <input
          type="checkbox"
          checked={draft.enabled}
          onChange={(event) => onChange({ ...draft, enabled: event.currentTarget.checked })}
        />
        {t("worldbookEnabled")}
      </label>
      <fieldset className="worldbook-visibility">
        <legend>{t("worldbookVisibility")}</legend>
        {(["gm", "public", "characters"] as const).map((visibility) => (
          <label className="inline" key={visibility}>
            <input
              type="radio"
              name="worldbook-visibility"
              value={visibility}
              checked={draft.visibility === visibility}
              onChange={() => {
                onChange({ ...draft, visibility });
                // 點「指定角色」當下重抓在場角色：畫面開著時可能剛從隱藏區還原角色
                if (visibility === "characters") onRefreshCharacters();
              }}
            />
            {visibility === "gm"
              ? t("worldbookVisibilityGm")
              : visibility === "public"
                ? t("worldbookVisibilityPublic")
                : t("worldbookVisibilityCharacters")}
          </label>
        ))}
      </fieldset>
      {draft.visibility === "characters" && (
        <fieldset className="worldbook-characters">
          <legend>{t("worldbookChooseCharacters")}</legend>
          {characters.length === 0 ? (
            <span>{t("worldbookNoCharacters")}</span>
          ) : (
            characters.map((character) => (
              <label className="inline" key={character.id}>
                <input
                  type="checkbox"
                  checked={draft.characters.includes(character.id)}
                  onChange={(event) =>
                    onChange({
                      ...draft,
                      characters: event.currentTarget.checked
                        ? [...draft.characters, character.id]
                        : draft.characters.filter((id) => id !== character.id),
                    })
                  }
                />
                {character.name}
              </label>
            ))
          )}
        </fieldset>
      )}
    </form>
  );
}
