import { useCallback, useEffect, useRef, useState } from "react";
import { MAX_WEB_SAVE_BYTES } from "@desktop/shared/contracts/web-save/web-save";
import { t, type MsgKey } from "../../i18n";
import { GLOBAL_VARIABLES, type GameSetup } from "../chat/useChat";
import type { ReleaseInfo } from "../funnel/releases";
import { downloadWebSave } from "./download";
import { ExportFunnel } from "./ExportFunnel";
import { isSafari, newSaveId, type SaveMeta, type SaveStore } from "./save-store";
import { adoptGlobals, restoreWebSaveText, type RestoreError } from "./web-save-codec";

function restoreMessage(error: RestoreError): string {
  if (error.kind === "card") return t(`importErr_${error.error}` as MsgKey);
  const save = error.error;
  if (save.kind === "version") return t("savesErr_version", { version: save.version });
  if (save.kind === "invalid") return t("savesErr_invalid", { detail: save.detail });
  return t("savesErr_not_web_save");
}

/**
 * 開始畫面的存檔區：這個瀏覽器裡的多份存檔（繼續、匯出、刪除）與匯入網頁存檔。Safari 的 ITP 會清資料，
 * 提示放在這裡（玩家看存檔的地方）。
 */
export function SavesPanel({
  saves,
  persisted,
  release,
  onContinue,
}: {
  saves: SaveStore | null;
  persisted: boolean | null;
  release: ReleaseInfo;
  onContinue: (game: GameSetup) => void;
}) {
  const [list, setList] = useState<SaveMeta[]>([]);
  const [error, setError] = useState<string | null>(saves ? null : t("savesErr_storage"));
  const [confirming, setConfirming] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [exported, setExported] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const refresh = useCallback(async () => {
    if (!saves) return;
    try {
      setList(await saves.list());
    } catch {
      setError(t("savesErr_storage"));
    }
  }, [saves]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const load = async (id: string) => {
    setError(null);
    const save = await saves?.get(id).catch(() => null);
    if (!save) return setError(t("savesErr_missing"));
    // 存檔庫裡的也照契約再驗一次（瀏覽器資料可能被別的程式改壞）
    const restored = restoreWebSaveText(JSON.stringify(save));
    if (!restored.ok) return setError(restoreMessage(restored.error));
    const { game } = restored;
    adoptGlobals(GLOBAL_VARIABLES, game.global);
    onContinue({
      card: game.card,
      userName: game.userName,
      openingIndex: game.openingIndex,
      saveId: id,
      resume: { entries: game.entries, local: game.local, carry: game.carry },
    });
  };

  const exportOne = async (meta: SaveMeta) => {
    setError(null);
    const save = await saves?.get(meta.id).catch(() => null);
    if (!save) return setError(t("savesErr_missing"));
    const failed = downloadWebSave(save, meta.title);
    if (failed) setError(t("exportFailed", { detail: failed }));
    else setExported(true);
  };

  const remove = async (id: string) => {
    setConfirming(null);
    await saves?.remove(id).catch(() => setError(t("savesErr_storage")));
    await refresh();
  };

  const importFile = async (file: File) => {
    setError(null);
    if (file.size > MAX_WEB_SAVE_BYTES) return setError(t("savesErr_too_large"));
    setImporting(true);
    try {
      const restored = restoreWebSaveText(await file.text());
      if (!restored.ok) return setError(restoreMessage(restored.error));
      const meta = {
        id: newSaveId(),
        title: restored.game.card.text.name,
        updatedAt: Date.now(),
        messageCount: restored.save.messages.length,
      };
      await saves?.put(meta, restored.save);
      await refresh();
    } catch {
      setError(t("savesErr_storage"));
    } finally {
      setImporting(false);
      if (fileRef.current) fileRef.current.value = "";
    }
  };

  const hint = isSafari() ? t("savesSafariHint") : persisted === false ? t("savesEvictHint") : null;

  return (
    <article className="card-tile panel saves" data-testid="saves">
      <div className="card-tile-head">
        <h2>{t("savesTitle")}</h2>
      </div>
      {list.length === 0 && <p>{t("savesEmpty")}</p>}
      <ul className="save-list">
        {list.map((meta) => (
          <li key={meta.id} className="save-item" data-testid="save-item">
            <div className="save-info">
              <strong>{meta.title}</strong>
              <span className="save-note">
                {t("savesMeta", { count: meta.messageCount, time: new Date(meta.updatedAt).toLocaleString("zh-TW") })}
              </span>
            </div>
            <div className="save-actions">
              {confirming === meta.id ? (
                <>
                  <button type="button" className="danger" onClick={() => void remove(meta.id)}>
                    {t("savesDeleteConfirm")}
                  </button>
                  <button type="button" className="ghost" onClick={() => setConfirming(null)}>
                    {t("savesDeleteCancel")}
                  </button>
                </>
              ) : (
                <>
                  <button type="button" className="primary" onClick={() => void load(meta.id)}>
                    {t("savesContinue")}
                  </button>
                  <button type="button" onClick={() => void exportOne(meta)}>
                    {t("savesExport")}
                  </button>
                  <button type="button" className="ghost" onClick={() => setConfirming(meta.id)}>
                    {t("savesDelete")}
                  </button>
                </>
              )}
            </div>
          </li>
        ))}
      </ul>
      {error && (
        <p className="chat-error" role="alert" data-testid="saves-error">
          {error}
        </p>
      )}
      {exported && <ExportFunnel release={release} onClose={() => setExported(false)} />}
      {saves && (
        <>
          <input
            ref={fileRef}
            type="file"
            accept=".json,application/json"
            hidden
            data-testid="save-file"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) void importFile(file);
            }}
          />
          <button type="button" disabled={importing} onClick={() => fileRef.current?.click()}>
            {importing ? t("savesImporting") : t("savesImport")}
          </button>
        </>
      )}
      {hint && <p className="save-note save-hint">{hint}</p>}
    </article>
  );
}
