// VN 開桌精靈骨架：① 起點、② 展開重用一句話開桌（GenerateTableDialog＋genesis，建出 vn 桌）；
// ① 也可以改選「我有角色卡」：交給匯入流程開 VN 桌，匯完回到這裡的 ②。
// 從匯入來的桌角色與世界書已就位，直接從 ② 起（聖經生成在後續版本）。③ 立繪挑定稿（VnSpriteStep），
// ④ 開演＝進桌，進桌後背景補表情。
// 精靈結束＝進那張 vn 桌；中途關掉時桌已建好、留在大廳清單裡。
import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";
import { VnSpriteStep } from "./VnSpriteStep";
import { t, type MsgKey } from "../../i18n";
import { Dialog } from "../../shared/ui/Dialog";
import { GenerateTableDialog } from "../lobby/GenerateTableDialog";

/** worldId＝null 從一句話起點開始；有值＝桌已建好（匯入來的），從 ② 起 */
export interface VnWizardStart {
  worldId: string | null;
}

type PlaceholderStep = "expand" | "sprites" | "start";

const STEPS = {
  expand: { title: "vnStepExpandTitle", body: "vnStepExpandImported", next: "sprites" },
  sprites: { title: "vnStepSpritesTitle", body: "vnStepSpritesBody", next: "start" },
  start: { title: "vnStepStartTitle", body: "vnStepStartBody", next: null },
} satisfies Record<PlaceholderStep, { title: MsgKey; body: MsgKey; next: PlaceholderStep | null }>;

export function VnWizard({
  start,
  transport,
  onCreated,
  onImportCard,
  onFinish,
  onClose,
}: {
  start: VnWizardStart;
  transport?: string;
  /** ② 展開建好桌：大廳清單重讀 */
  onCreated: (worldId: string) => Promise<void>;
  /** ①「我有角色卡」選了檔：開 VN 桌並匯入，匯完以該桌從 ② 重開精靈 */
  onImportCard: (file: File) => void;
  /** ④ 按開始：進那張桌 */
  onFinish: (worldId: string) => void;
  onClose: () => void;
}) {
  const [step, setStep] = useState<"genesis" | PlaceholderStep>(
    start.worldId === null ? "genesis" : "expand",
  );
  // GenerateTableDialog 建好桌後會接著呼叫 onClose：用 ref 同步記下，別讓那一下把整個精靈關掉
  const worldId = useRef(start.worldId);
  const cardInput = useRef<HTMLInputElement>(null);
  const [spritesReady, setSpritesReady] = useState(false);

  if (step === "genesis") {
    return (
      <GenerateTableDialog
        open
        tableKind="vn"
        title={t("vnWizardTitle")}
        transport={transport}
        toolbar={(busy) => (
          <>
            <button
              type="button"
              className="btn"
              disabled={busy}
              onClick={() => cardInput.current?.click()}
            >
              {t("vnHaveCard")}
            </button>
            <input
              ref={cardInput}
              type="file"
              accept=".png,.json,image/png,application/json"
              hidden
              onChange={(event) => {
                const file = event.currentTarget.files?.[0];
                event.currentTarget.value = "";
                if (file) onImportCard(file);
              }}
            />
          </>
        )}
        onCreated={async (id) => {
          worldId.current = id;
          setStep("sprites");
          await onCreated(id);
        }}
        onClose={() => {
          if (worldId.current === null) onClose();
        }}
      />
    );
  }

  const current = STEPS[step];
  const next: PlaceholderStep | null = current.next;
  const advance = () => {
    if (next !== null) setStep(next);
    else if (worldId.current !== null) {
      // ④ 開演：開始在背景補表情（播放器可能早已掛著，不能只靠它掛載時排）
      invoke("vn_sprites_fill", { worldId: worldId.current }).catch(() => {});
      onFinish(worldId.current);
    }
  };
  const sprites = step === "sprites" && worldId.current !== null;
  return (
    <Dialog
      title={t(current.title)}
      className="vn-wizard"
      onDismiss={onClose}
      closeButton
      end={
        <>
          {sprites && (
            <button type="button" className="btn" onClick={advance}>
              {t("vnSpriteSkipStep")}
            </button>
          )}
          <button
            type="button"
            className="btn btn-primary"
            disabled={sprites && !spritesReady}
            onClick={advance}
          >
            {t(next !== null ? "vnNext" : "vnStart")}
          </button>
        </>
      }
    >
      {sprites ? (
        <VnSpriteStep worldId={worldId.current!} onReady={setSpritesReady} />
      ) : (
        <p>{t(current.body)}</p>
      )}
    </Dialog>
  );
}
