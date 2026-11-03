// 精靈 ③ 立繪：桌上每位角色（不含玩家、隱藏與舞台卡）生成定稿候選、可再骰、挑定；一鍵全部生成。
// 全部挑定才能下一步，也可略過（沒定稿＝舞台用角色卡原圖）。表情差分等 ④ 開演進桌後在背景補。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import type { CharacterMeta } from "../characters/card-model";
import { SpriteBasePicker } from "./SpriteBasePicker";
import { useSpriteSlot } from "./useSpriteSlot";

export function VnSpriteStep({
  worldId,
  onReady,
}: {
  worldId: string;
  onReady: (ready: boolean) => void;
}) {
  const [cast, setCast] = useState<CharacterMeta[] | null>(null);
  const [picked, setPicked] = useState<Record<string, boolean>>({});
  const [allTick, setAllTick] = useState(0);
  useEffect(() => {
    invoke<CharacterMeta[]>("list_characters", { worldId })
      .then((list) => setCast(list.filter((card) => !card.archived && !card.stage)))
      .catch(() => setCast([]));
  }, [worldId]);
  useEffect(() => {
    onReady(cast !== null && cast.every((card) => picked[card.id]));
  }, [cast, picked, onReady]);
  return (
    <div className="vn-sprite-step">
      <p>{t("vnStepSpritesBody")}</p>
      {cast && cast.length > 1 && (
        <button type="button" className="btn btn-sm" onClick={() => setAllTick((tick) => tick + 1)}>
          {t("vnSpriteGenerateAll")}
        </button>
      )}
      {cast?.map((card) => (
        <SpriteStepRow
          key={card.id}
          worldId={worldId}
          card={card}
          allTick={allTick}
          onPicked={(done) => setPicked((current) => ({ ...current, [card.id]: done }))}
        />
      ))}
    </div>
  );
}

function SpriteStepRow({
  worldId,
  card,
  allTick,
  onPicked,
}: {
  worldId: string;
  card: CharacterMeta;
  allTick: number;
  onPicked: (done: boolean) => void;
}) {
  const slot = useSpriteSlot(worldId, card.id);
  const done = Boolean(slot.state?.sprite);
  useEffect(() => onPicked(done), [done]);
  useEffect(() => {
    // 全部生成：還沒定稿也還沒候選的才生（後端同一來源一次一張，排隊依序出圖）
    if (allTick > 0 && slot.state && !done && slot.state.candidates.length === 0) {
      void slot.generate();
    }
  }, [allTick]);
  return (
    <section className="vn-sprite-row">
      <h3>
        {card.name}
        {done && <span className="vn-sprite-done">{t("vnSpritePicked")}</span>}
      </h3>
      <SpriteBasePicker slot={slot} name={card.name} />
    </section>
  );
}
