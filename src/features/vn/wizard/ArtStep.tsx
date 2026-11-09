// 精靈 ③ 美術準備（合併舊 ③ 立繪與 ④ 開演）：頂部一行額度進度與立繪來源，每位角色一列候選（再骰、挑定、上傳），
// 右側首幕背景。桌一建好就掛著（精靈層隱藏掛載），掛上就排後端自動定稿（冪等）：沒圖的角色在背景生候選並裝成定稿，
// ③ 只讀狀態。主鈕永遠可按：選了別張候選的以那張為定稿；再骰中的交給後端再骰完用新圖；還沒出圖的進桌後續生。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type MutableRefObject } from "react";
import { t } from "../../../i18n";
import type { Backdrop } from "../../../shared/contracts/backend-contracts";
import type { CharacterMeta } from "../../characters/card-model";
import { SpriteBasePicker } from "../SpriteBasePicker";
import { backdropKey, useBackgrounds } from "../useBackgrounds";
import { spriteError, useSpriteSlot, type SpriteSlot } from "../useSpriteSlot";
import { ArtFlowNotice } from "../art-flow/ArtFlowNotice";
import "../vn-sprites.css";

export interface ArtProgress {
  started: boolean;
  ready: number;
  total: number;
}

/** 已安裝定稿（與標題畫面的「立繪準備中 n/N」同一個定義） */
const installed = (slot: SpriteSlot) => Boolean(slot.state?.sprite);

/** 目前顯示的候選不是裝上去那張就裝它（沒選＝顯示的就是定稿，不動） */
async function finalize(slot: SpriteSlot) {
  if (!slot.selected || (slot.state?.sprite && slot.selected === slot.installed)) return;
  await invoke("vn_sprite_pick", { worldId: slot.worldId, characterId: slot.characterId, file: slot.selected });
}

function FirstBackdrop({ worldId }: { worldId: string }) {
  const [first, setFirst] = useState<Backdrop | null>(null);
  const { images, statusOf, load, pause } = useBackgrounds(worldId);
  useEffect(() => {
    invoke<Backdrop[]>("vn_backgrounds_fill", { worldId, scene: null })
      .then((used) => setFirst(used[0] ?? null))
      .catch(() => {});
    invoke("vn_audio_fill", { worldId, scene: null }).catch(() => {});
  }, [worldId]);
  useEffect(() => {
    if (first) load(first);
  }, [first, load]);
  const key = first && backdropKey(first);
  const src = key ? images[key]?.src : null;
  return (
    <aside className="vn-art-backdrop">
      <h3>{t("vnArtBackdrop")}</h3>
      {src ? (
        <img className="vn-start-preview" src={src} alt={first?.location} />
      ) : (
        <div className="vn-start-preview vn-start-placeholder">
          <p className="vn-start-progress">
            {t(pause ? "vnBackgroundWaitQuota" : first && statusOf(first)?.state === "failed" ? "vnBackgroundFailed" : first && statusOf(first) ? "vnBackgroundPreparing" : "vnBackgroundEmpty")}
          </p>
        </div>
      )}
    </aside>
  );
}

function ArtRow({
  worldId,
  card,
  slots,
  failed,
  onChange,
  paused,
}: {
  worldId: string;
  paused: boolean;
  card: CharacterMeta;
  slots: MutableRefObject<Map<string, SpriteSlot>>;
  /** 按開始時把目前候選裝成定稿失敗的原因 */
  failed: string | undefined;
  onChange: () => void;
}) {
  const slot = useSpriteSlot(worldId, card.id);
  slots.current.set(card.id, slot);
  const picked = Boolean(slot.state?.sprite);
  useEffect(onChange, [slot.state, slot.selected]);
  return (
    <section className="vn-sprite-row">
      <h3>
        {card.name}
        {picked && <span className="vn-sprite-done">{t("vnSpritePicked")}</span>}
      </h3>
      <SpriteBasePicker slot={slot} name={card.name} source={false} paused={paused} />
      {failed && (
        <p className="field-warn" role="alert">
          {t("vnArtInstallFailed", { reason: failed })}
        </p>
      )}
    </section>
  );
}

export function ArtStep({
  worldId,
  active,
  onProgress,
  finishRef,
}: {
  worldId: string;
  /** 正顯示中；首幕背景第一次顯示才排 */
  active: boolean;
  onProgress: (progress: ArtProgress) => void;
  /** 精靈按「開始遊戲」時呼叫：把沒挑定的角色裝成定稿 */
  finishRef: MutableRefObject<(() => Promise<boolean>) | null>;
}) {
  const [paused, setPaused] = useState(false);
  const [started, setStarted] = useState(false);
  const [cast, setCast] = useState<CharacterMeta[] | null>(null);
  const [, setTick] = useState(0);
  const [failed, setFailed] = useState<Record<string, string>>({});
  const [shown, setShown] = useState(active);
  const slots = useRef(new Map<string, SpriteSlot>());

  useEffect(() => {
    if (active) setShown(true);
  }, [active]);
  useEffect(() => {
    invoke<CharacterMeta[]>("list_characters", { worldId })
      .then((list) => setCast(list.filter((card) => !card.archived && !card.stage)))
      .catch(() => setCast([]));
  }, [worldId]);

  const list = cast?.map((card) => slots.current.get(card.id)).filter((slot) => slot !== undefined) ?? [];
  const ready = list.filter(installed).length;
  useEffect(() => {
    if (cast !== null) onProgress({ started, ready, total: cast.length });
  }, [started, ready, cast, onProgress]);

  /** 選了別張的裝成定稿、再骰中的交給後端；有失敗就留在 ③ 顯示在該列（再按開始＝重試），回 false */
  finishRef.current = async () => {
    const errors: Record<string, string> = {};
    const rerolling: string[] = [];
    for (const [id, slot] of slots.current) {
      if (slot.busy === "generate") {
        rerolling.push(id);
        continue;
      }
      await finalize(slot).catch((reason) => (errors[id] = spriteError(reason)));
      await slot.reload().catch(() => {});
    }
    if (rerolling.length) await invoke("vn_sprites_prepare", { worldId, adopt: rerolling });
    setFailed(errors);
    return Object.keys(errors).length === 0;
  };

  return (
    <div className="vn-art-step">
      <div className="vn-art-head">
        <ArtFlowNotice worldId={worldId} onStarted={setStarted} onPause={setPaused} />
      </div>
      <div className="vn-art-body">
        <div className="vn-sprite-step">
          {cast?.map((card) => (
            <ArtRow
              key={card.id}
              worldId={worldId}
              card={card}
              paused={paused}
              slots={slots}
              failed={failed[card.id]}
              onChange={() => setTick((n) => n + 1)}
            />
          ))}
        </div>
        {shown && <FirstBackdrop worldId={worldId} />}
      </div>
    </div>
  );
}
