// 存檔與讀檔（vn-mode 包 6 §2.7）：存檔＝在目前這格放一個命名書籤；讀檔＝回到那格（目前幕直接跳、別條分支切過去、
// 已換章的幕分岔一份）。標題畫面與快捷列共用。
import { useState } from "react";
import { t } from "../../i18n";
import type { VnSave } from "../../shared/contracts/backend-contracts";
import { Panel } from "./StoryPanels";

export function SavesPanel({
  label,
  saves,
  defaultName,
  canSave,
  disabled,
  error,
  onSave,
  onLoad,
  onDelete,
  onClose,
}: {
  /** 標題畫面只讀檔時叫「讀檔」 */
  label: string;
  saves: VnSave[];
  defaultName: string;
  /** 標題畫面只讀檔 */
  canSave: boolean;
  disabled: boolean;
  error: string | null;
  onSave: (name: string) => void;
  onLoad: (id: string) => void;
  onDelete: (id: string) => void;
  onClose: () => void;
}) {
  const [name, setName] = useState(defaultName);
  return (
    <Panel label={label} onClose={onClose}>
      {canSave && (
        <form
          className="vn-save-form"
          onSubmit={(event) => {
            event.preventDefault();
            onSave(name);
          }}
        >
          <input
            className="vn-save-name"
            aria-label={t("vnSaveName")}
            value={name}
            maxLength={60}
            onChange={(event) => setName(event.target.value)}
          />
          <button type="submit" className="btn btn-primary" disabled={disabled || !name.trim()}>
            {t("vnSaveCreate")}
          </button>
        </form>
      )}
      {error && (
        <p className="vn-choice-error" role="alert">
          {error}
        </p>
      )}
      {saves.length === 0 ? (
        <p className="vn-chapter-note">{t("vnSaveEmpty")}</p>
      ) : (
        <ol className="vn-save-list">
          {[...saves].reverse().map((save) => (
            <li key={save.id}>
              <div className="vn-save-meta">
                <b>{save.name}</b>
                <span>
                  {t("vnSaveChapter", { n: save.chapter })} · {save.ts}
                </span>
                {save.preview && <span className="vn-save-preview">{save.preview}</span>}
              </div>
              <button type="button" className="btn" disabled={disabled} onClick={() => onLoad(save.id)}>
                {t("vnSaveLoad")}
              </button>
              <button type="button" className="btn btn-ghost" disabled={disabled} onClick={() => onDelete(save.id)}>
                {t("vnSaveDelete")}
              </button>
            </li>
          ))}
        </ol>
      )}
    </Panel>
  );
}
