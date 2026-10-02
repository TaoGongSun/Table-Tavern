// 陣容欄的卡片：寬欄的橫式桌遊組件卡、窄欄的頭像格。
// 兩種外觀共用同一套語意：主按鈕＝選為發言對象（aria-pressed）、旁邊獨立的鉛筆鈕＝進編輯，
// 兩顆鈕是同層兄弟（HTML 不能巢狀 button）；玩家卡例外，整張一顆鈕＝編輯／建玩家卡。
// 描邊與 aria-pressed 只看發言對象，編輯畫面開著也不改。
import type { CSSProperties } from "react";
import { t } from "../i18n";
import { tierLabel } from "../features/ai-connection/model-catalog";
import type { CharacterCard, CharacterMeta } from "../features/characters/card-model";
import { IconEdit, IconPlus } from "../shared/ui/icons";
import gmBook from "../assets/gm-book.png";

type DragProps = Record<string, unknown>;

function facStyle(color: string): CSSProperties {
  return { ["--fac" as string]: color };
}

/** 角色圖窗的內容：全身圖優先，其次頭像，最後 emoji */
function CastArt({
  meta,
  image,
  avatar,
}: {
  meta: CharacterMeta | CharacterCard;
  image: string | undefined | null;
  avatar: string | undefined | null;
}) {
  if (meta.show_image && image) return <img className="tcard-image" src={image} alt="" />;
  if (avatar) return <img className="avatar-round tcard-avatar" src={avatar} alt="" />;
  return <span aria-hidden="true">{meta.avatar}</span>;
}

function EditButton({
  label,
  disabled,
  onClick,
  className,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
  className: string;
}) {
  return (
    <button
      type="button"
      className={`btn btn-ghost btn-icon ${className}`}
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
    >
      <IconEdit />
    </button>
  );
}

// ---------- 寬欄 ----------

export function WideGmCard({
  selected,
  image,
  locked,
  onSelect,
  onEdit,
}: {
  selected: boolean;
  image: string | null;
  locked: boolean;
  onSelect: () => void;
  onEdit: () => void;
}) {
  return (
    <div className={`tcard tcard-gm${selected ? " tcard-selected" : ""}`}>
      <button
        type="button"
        className="tcard-main"
        aria-pressed={selected}
        title={t(selected ? "gmTargetHintClear" : "gmTargetHint")}
        onClick={onSelect}
      >
        <span className="tcard-art">
          {image ? (
            <img className="tcard-image" src={image} alt="" />
          ) : (
            <img className="gm-book" src={gmBook} alt="" />
          )}
        </span>
        <span className="tcard-body">
          <span className="tcard-name-row">
            <span className="tcard-plate">GM</span>
          </span>
        </span>
      </button>
      <EditButton
        label={t("worldSummary")}
        disabled={locked}
        onClick={onEdit}
        className="tcard-edit"
      />
    </div>
  );
}

export function WidePlayerCard({
  player,
  image,
  avatar,
  locked,
  onOpen,
}: {
  player: CharacterCard | null;
  image: string | null;
  avatar: string | null;
  locked: boolean;
  onOpen: () => void;
}) {
  return (
    <button
      type="button"
      className={`tcard tcard-player tcard-player-btn${player ? "" : " tcard-player-empty"}`}
      title={t(player ? "playerCardHint" : "playerCardEmptyHint")}
      disabled={locked}
      onClick={onOpen}
    >
      {player ? (
        <>
          <span className="tcard-art">
            <CastArt meta={player} image={image} avatar={avatar} />
          </span>
          <span className="tcard-body">
            <span className="tcard-name-row">
              <span className="tcard-plate">{player.name}</span>
            </span>
          </span>
        </>
      ) : (
        <span className="tcard-body">{t("playerCardEmpty")}</span>
      )}
    </button>
  );
}

export function WideCastCard({
  character,
  selected,
  dragging,
  image,
  avatar,
  locked,
  dragProps,
  justDragged,
  onSelect,
  onEdit,
}: {
  character: CharacterMeta;
  selected: boolean;
  dragging: boolean;
  image: string | undefined;
  avatar: string | undefined;
  locked: boolean;
  /** 拖曳排序掛外層卡片；locked 時整個排序停用，不傳 */
  dragProps: DragProps | null;
  justDragged: () => boolean;
  onSelect: () => void;
  onEdit: () => void;
}) {
  const name = character.name;
  return (
    <div
      className={`tcard${selected ? " tcard-selected" : ""}${dragging ? " row-dragging" : ""}`}
      style={facStyle(character.color)}
      {...dragProps}
    >
      <button
        type="button"
        className="tcard-main"
        aria-pressed={selected}
        data-drag-handle
        title={`${t(selected ? "castHintClear" : "castHint", { name })}｜${t("dragToReorder")}`}
        disabled={locked}
        onClick={() => {
          if (!justDragged()) onSelect();
        }}
      >
        <span className="tcard-art">
          <CastArt meta={character} image={image} avatar={avatar} />
        </span>
        <span className="tcard-body">
          <span className="tcard-name-row">
            <span className="tcard-plate">{name}</span>
            {character.tier !== "balanced" && (
              <span className="tcard-gem">{tierLabel(character.tier)}</span>
            )}
          </span>
        </span>
      </button>
      <EditButton
        label={t("editCardSummary", { name })}
        disabled={locked}
        onClick={onEdit}
        className="tcard-edit"
      />
    </div>
  );
}

// ---------- 窄欄 ----------

export function NarrowGmTile({
  selected,
  image,
  locked,
  onSelect,
  onEdit,
}: {
  selected: boolean;
  image: string | null;
  locked: boolean;
  onSelect: () => void;
  onEdit: () => void;
}) {
  return (
    <div className="rail-tile">
      <button
        type="button"
        className="rail-tile-main"
        aria-label="GM"
        aria-pressed={selected}
        title={t(selected ? "gmTargetHintClear" : "gmTargetHint")}
        onClick={onSelect}
      >
        <span className={`rail-tile-art rail-tile-gm${selected ? " rail-tile-selected" : ""}`}>
          <img src={image ?? gmBook} className={image ? "tcard-image" : "gm-book"} alt="" />
        </span>
        <span className="rail-tile-name" aria-hidden="true">
          GM
        </span>
      </button>
      <EditButton
        label={t("worldSummary")}
        disabled={locked}
        onClick={onEdit}
        className="rail-tile-edit"
      />
    </div>
  );
}

export function NarrowPlayerTile({
  player,
  image,
  avatar,
  locked,
  onOpen,
}: {
  player: CharacterCard | null;
  image: string | null;
  avatar: string | null;
  locked: boolean;
  onOpen: () => void;
}) {
  const label = player?.name ?? t("playerCardEmpty");
  return (
    <div className="rail-tile">
      <button
        type="button"
        className="rail-tile-main"
        aria-label={label}
        title={t(player ? "playerCardHint" : "playerCardEmptyHint")}
        disabled={locked}
        onClick={onOpen}
      >
        <span className="rail-tile-art rail-tile-player">
          {player ? <CastArt meta={player} image={image} avatar={avatar} /> : <IconPlus />}
        </span>
        <span className="rail-tile-name" aria-hidden="true">
          {player?.name ?? t("playerCardShort")}
        </span>
      </button>
    </div>
  );
}

export function NarrowCastTile({
  character,
  selected,
  image,
  avatar,
  locked,
  onSelect,
  onEdit,
}: {
  character: CharacterMeta;
  selected: boolean;
  image: string | undefined;
  avatar: string | undefined;
  locked: boolean;
  onSelect: () => void;
  onEdit: () => void;
}) {
  const name = character.name;
  return (
    <div className="rail-tile" style={facStyle(character.color)}>
      <button
        type="button"
        className="rail-tile-main"
        aria-label={name}
        aria-pressed={selected}
        title={t(selected ? "castHintClear" : "castHint", { name })}
        disabled={locked}
        onClick={onSelect}
      >
        <span className={`rail-tile-art${selected ? " rail-tile-selected" : ""}`}>
          <CastArt meta={character} image={image} avatar={avatar} />
          {character.tier !== "balanced" && (
            <span className="tcard-gem rail-tile-gem">{tierLabel(character.tier)}</span>
          )}
        </span>
        <span className="rail-tile-name" aria-hidden="true">
          {name}
        </span>
      </button>
      <EditButton
        label={t("editCardSummary", { name })}
        disabled={locked}
        onClick={onEdit}
        className="rail-tile-edit"
      />
    </div>
  );
}
