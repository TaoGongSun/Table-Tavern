// 精靈 ③ 美術準備（合併舊 ③ 立繪與 ④ 開演）：頂部一行額度進度與立繪來源，每位角色一列候選（再骰、挑定、上傳），
// 右側首幕背景。桌一建好就掛著（精靈層隱藏掛載）：候選在建桌完成時排一次、一次一張，返回 ③ 只接續不重排。
// 主鈕永遠可按：沒挑定的角色以目前顯示那張安裝為定稿；還沒出圖的進桌後由包 B 的後端工作續生。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type MutableRefObject } from "react";
import { t } from "../../../i18n";
import type { Backdrop } from "../../../shared/contracts/backend-contracts";
import type { CharacterMeta } from "../../characters/card-model";
import { SpriteBasePicker, SpriteSourceSelect } from "../SpriteBasePicker";
import { backdropKey, useBackgrounds } from "../useBackgrounds";
import { spriteError, useSpriteSlot, type SpriteSlot } from "../useSpriteSlot";
import "../vn-sprites.css";

export interface ArtProgress {
  ready: number;
  total: number;
}

/** 這位角色還沒有任何圖（沒定稿也沒候選）：排隊要生 */
const needsCandidate = (slot: SpriteSlot) =>
  slot.state !== null && !slot.state.sprite && slot.state.candidates.length === 0;
/** 已安裝定稿（與標題畫面的「立繪準備中 n/N」同一個定義） */
const installed = (slot: SpriteSlot) => Boolean(slot.state?.sprite);

/** 沒挑定的角色把目前顯示的候選裝成定稿（挑過或上傳過的不動） */
async function finalize(slot: SpriteSlot) {
  if (slot.state?.sprite || !slot.selected) return;
  await invoke("vn_sprite_pick", { worldId: slot.worldId, characterId: slot.characterId, file: slot.selected });
}

function FirstBackdrop({ worldId }: { worldId: string }) {
  const [first, setFirst] = useState<Backdrop | null>(null);
  const { images, status, load } = useBackgrounds(worldId);
  useEffect(() => {
    invoke<Backdrop[]>("vn_backgrounds_fill", { worldId, scene: null })
      .then((used) => setFirst(used[0] ?? null))
      .catch(() => {});
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
            {t(key && status[key]?.state === "failed" ? "vnBackgroundFailed" : "vnBackgroundPreparing")}
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
}: {
  worldId: string;
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
      <SpriteBasePicker slot={slot} name={card.name} source={false} />
      {failed && !picked && (
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
  const [cast, setCast] = useState<CharacterMeta[] | null>(null);
  const [, setTick] = useState(0);
  const [failed, setFailed] = useState<Record<string, string>>({});
  const [shown, setShown] = useState(active);
  const slots = useRef(new Map<string, SpriteSlot>());
  const queued = useRef(false);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  useEffect(() => {
    if (active) setShown(true);
  }, [active]);
  useEffect(() => {
    invoke<CharacterMeta[]>("list_characters", { worldId })
      .then((list) => setCast(list.filter((card) => !card.archived && !card.stage)))
      .catch(() => setCast([]));
  }, [worldId]);

  const list = cast?.map((card) => slots.current.get(card.id)).filter((slot) => slot !== undefined) ?? [];
  const loaded = cast !== null && list.length === cast.length && list.every((slot) => slot.state !== null);
  const ready = list.filter(installed).length;
  useEffect(() => {
    if (cast !== null) onProgress({ ready, total: cast.length });
  }, [ready, cast, onProgress]);

  // 候選排一次：全部角色讀回狀態後，沒有任何圖的依序生（一次一張，等上一張回來再排下一張）
  useEffect(() => {
    if (!loaded || queued.current || cast === null) return;
    queued.current = true;
    void (async () => {
      for (const card of cast) {
        const slot = slots.current.get(card.id);
        if (!alive.current) return;
        if (slot && needsCandidate(slot)) await slot.generate();
      }
    })();
  }, [loaded]);

  /** 沒挑定的裝成定稿；有失敗就留在 ③ 顯示在該列（再按開始＝重試），回 false */
  finishRef.current = async () => {
    const errors: Record<string, string> = {};
    for (const [id, slot] of slots.current) {
      await finalize(slot).catch((reason) => (errors[id] = spriteError(reason)));
      await slot.reload().catch(() => {});
    }
    setFailed(errors);
    return Object.keys(errors).length === 0;
  };

  const pending = cast === null ? 0 : cast.length - ready;
  return (
    <div className="vn-art-step">
      <div className="vn-art-head">
        <p>{pending > 0 ? t("vnArtQuota", { n: cast?.length ?? 0 }) : t("vnArtReady")}</p>
        <SpriteSourceSelect />
      </div>
      <div className="vn-art-body">
        <div className="vn-sprite-step">
          {cast?.map((card) => (
            <ArtRow
              key={card.id}
              worldId={worldId}
              card={card}
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
