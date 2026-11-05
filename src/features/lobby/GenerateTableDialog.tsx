// 一句話開桌（聊天桌）：把玩家的一句話（＋類型標籤）交給 AI 產出世界觀綱要與角色名單，就地改完再展開成一張真的桌。
// 草稿與生成流程在 useTableGenesis；關閉只是不畫（open=false 時回 null，元件不卸載），草稿留著。
// 生成中不能關（× 停用、Esc 無效）；點遮罩一律不關，免得誤觸丟掉整份草稿。VN 桌走 features/vn/wizard。
import { t } from "../../i18n";
import { Dialog, SwapLabel } from "../../shared/ui/Dialog";
import { GenesisError, OutlinePreview } from "./OutlinePreview";
import { useTableGenesis } from "./useTableGenesis";

const GENRE_KEYS = [
  "genGenreFantasy",
  "genGenreScifi",
  "genGenreUrban",
  "genGenreWuxia",
  "genGenreSchool",
  "genGenreApocalypse",
] as const;

export function GenerateTableDialog({
  open,
  onClose,
  onCreated,
  transport,
}: {
  open: boolean;
  onClose: () => void;
  /** 桌生出來了：桌次清單重讀並進去那張新桌 */
  onCreated: (worldId: string) => Promise<void>;
  /** 目前的 AI 來源：失敗訊息要指對地方（API 換金鑰／CLI 重新登入） */
  transport?: string;
}) {
  const genesis = useTableGenesis("chat");
  if (!open) return null;

  const { busy, hasOutline } = genesis;
  const labels = genesis.genres.map((key) => t(key as (typeof GENRE_KEYS)[number]));
  const empty = !genesis.input.trim() && genesis.genres.length === 0;
  async function create() {
    const worldId = await genesis.createTable(labels);
    if (worldId === null) return;
    await onCreated(worldId);
    onClose();
  }
  return (
    <Dialog
      title={t("genTitle")}
      size="l"
      className="gen-table-dialog"
      onDismiss={busy === null ? onClose : undefined}
      closeButton
      start={
        hasOutline && (
          <button
            type="button"
            className="btn"
            disabled={busy !== null}
            onClick={() => void genesis.generateOutline(labels)}
          >
            {t("genRerollBtn")}
          </button>
        )
      }
      end={
        hasOutline && (
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy !== null || !genesis.canCreate}
            onClick={() => void create()}
          >
            <SwapLabel labels={[t("genCreateBtn"), t("genExpanding")]} current={busy === "expand" ? 1 : 0} />
          </button>
        )
      }
    >
      <textarea
        data-autofocus=""
        rows={4}
        value={genesis.input}
        placeholder={t("genInputPlaceholder")}
        aria-label={t("genInputPlaceholder")}
        disabled={busy !== null}
        onChange={(event) => genesis.setInput(event.currentTarget.value)}
      />
      <div className="gen-genres">
        {GENRE_KEYS.map((key) => {
          const selected = genesis.genres.includes(key);
          return (
            <button
              key={key}
              type="button"
              className={`gen-genre${selected ? " gen-genre-selected" : ""}`}
              aria-pressed={selected}
              disabled={busy !== null}
              onClick={() =>
                genesis.setGenres((current) =>
                  selected ? current.filter((genre) => genre !== key) : [...current, key],
                )
              }
            >
              {t(key)}
            </button>
          );
        })}
      </div>
      {/* 有綱要後底部「建立」接手主鈕，這顆降成次鈕但留在原位 */}
      <div className="gen-generate-row">
        <button
          type="button"
          className={hasOutline ? "btn" : "btn btn-primary"}
          disabled={busy !== null || empty}
          onClick={() => void genesis.generateOutline(labels)}
        >
          <SwapLabel labels={[t("genGenerateBtn"), t("genGenerating")]} current={busy === "outline" ? 1 : 0} />
        </button>
        <small>{t("genQuotaNote")}</small>
      </div>
      <OutlinePreview genesis={genesis} genreLabels={labels} />
      <GenesisError genesis={genesis} transport={transport} />
    </Dialog>
  );
}
