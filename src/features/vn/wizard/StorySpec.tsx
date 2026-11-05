// 精靈 ② 的「劇本規格」層：劇透鎖住時仍露出可改的章數、選擇程度（附結局數）與角色公開介紹；
// 章標題、節拍、秘密、旗標、結局條件留在遮罩內（BibleTab 的 SpoilerMask），「重寫劇本」沿用。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type MutableRefObject } from "react";
import { t } from "../../../i18n";
import type { CharacterCard, CharacterMeta } from "../../characters/card-model";
import { BibleTab } from "../bible/BibleTab";
import { useBible } from "../bible/useBible";

function CastIntro({ worldId }: { worldId: string }) {
  const [cast, setCast] = useState<CharacterCard[]>([]);
  useEffect(() => {
    let live = true;
    invoke<CharacterMeta[]>("list_characters", { worldId })
      .then((list) =>
        Promise.all(
          list
            .filter((card) => !card.archived && !card.stage)
            .map((card) => invoke<CharacterCard>("read_character", { worldId, characterId: card.id })),
        ),
      )
      .then((cards) => live && setCast(cards))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [worldId]);
  if (cast.length === 0) return null;
  return (
    <div className="vn-cast-intro">
      <h3>{t("vnCastIntro")}</h3>
      <ul>
        {cast.map((card) => (
          <li key={card.id}>
            <b>
              {card.avatar} {card.name}
            </b>
            <p>{card.public_md}</p>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function StorySpec({
  worldId,
  onDirtyChange,
  flushRef,
}: {
  worldId: string;
  onDirtyChange: (dirty: boolean) => void;
  flushRef: MutableRefObject<(() => Promise<boolean>) | null>;
}) {
  const { locked } = useBible(worldId);
  return (
    <section className="vn-story-spec">
      <h3>{t("vnStorySpec")}</h3>
      {locked && <p className="vn-bible-note">{t("vnStorySpecLocked")}</p>}
      <BibleTab worldId={worldId} setup onDirtyChange={onDirtyChange} flushRef={flushRef} />
      <CastIntro worldId={worldId} />
    </section>
  );
}
