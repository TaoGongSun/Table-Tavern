// 角色域 controller：這桌的角色名單、本幕出場集合、玩家卡，以及角色圖／頭像／GM 圖三份快取。
// 所有權從 App() 搬過來；發言對象（speaker）留在 App，這裡只回報「這張卡沒了，該換誰」。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { t } from "../i18n";
import { isCharacterHidden } from "../character-visibility";
import { type WorldState } from "../backend-contracts";
import { type CharacterCard, type CharacterMeta } from "../card-model";

export interface CharacterController {
  /** 這桌的完整名單（含隱藏區），順序＝側欄順序 */
  list: CharacterMeta[];
  /** 側欄主區：沒被封存、且本幕出場過的 auto_hidden 卡也算 */
  active: CharacterMeta[];
  /** 側欄隱藏區 */
  archived: CharacterMeta[];
  /** 角色 id → 原圖 data URL（來源是匯入時存下的 PNG） */
  images: Record<string, string>;
  /** 角色 id → 頭像 data URL */
  avatars: Record<string, string>;
  player: CharacterCard | null;
  playerImage: string | null;
  playerAvatar: string | null;
  /** GM 卡的圖：世界書匯入的是 PNG 卡時後端存下的那張，null＝回退內建書本圖 */
  gmImage: string | null;
  metaOf: (id: string) => CharacterMeta | undefined;
  /** 換桌：名單、出場集合、玩家卡 id 一次同步塞進來，並清掉上一桌的圖（不得在裡面 await） */
  hydrate: (cast: CharacterMeta[], appearances: Set<string>, playerCardId: string | null) => void;
  /** 重讀名單與角色圖；worldId 省略＝目前這桌（開新桌並匯入時要顯式帶新桌 id） */
  refresh: (worldId?: string) => Promise<CharacterMeta[]>;
  /** 卡片編輯器存了新圖：名單沒變，載入 effect 不會自己重跑，這裡明著重讀 */
  reloadImages: () => Promise<void>;
  reloadGmImage: (worldId?: string) => Promise<void>;
  /** 玩家卡換人或存檔後重讀（同時記住這桌的玩家卡 id） */
  reloadPlayer: (playerCardId: string | null) => Promise<void>;
  /** 這輪劇情帶出場的卡：併入本幕出場集合，auto_hidden 卡立刻從隱藏區移回主區 */
  onArrived: (ids: string[]) => void;
  reorder: (ordered: CharacterMeta[]) => Promise<void>;
  restore: (id: string) => Promise<void>;
  restoreAutoHidden: (id: string) => Promise<void>;
  /** 角色被隱藏或刪除後的名單善後：回傳發言對象該換成誰，null＝發言對象不是它、不必動 */
  noteRemoved: (removedId: string, speaker: string) => Promise<string | null>;
  /** 確認框＋刪檔，true＝真的刪掉了（取消或失敗都是 false，畫面不用動） */
  remove: (id: string) => Promise<boolean>;
  removePlayer: (id: string) => Promise<boolean>;
}

export function useCharacterController(input: {
  worldId: string;
  onError: (message: string) => void;
}): CharacterController {
  const { worldId, onError } = input;
  const [characters, setCharacters] = useState<CharacterMeta[]>([]);
  // 本幕出場集合：換桌／切幕由 enterTable 呼叫 scene_appearances 初始化，
  // 之後每次 gm_narrate 回傳的 arrived_characters 併入——auto_hidden 卡一登場就立刻從隱藏區移回主區
  const [sceneAppearances, setSceneAppearances] = useState<Set<string>>(new Set());
  // 這桌的玩家卡 id（來自 world state）：玩家卡的載入 effect 靠它重跑
  const [playerCardId, setPlayerCardId] = useState<string | null>(null);
  const [playerCard, setPlayerCard] = useState<CharacterCard | null>(null);
  // 角色圖快取：角色 id → data URL（來源是匯入時存下的原 PNG，後端 read_character_image）
  const [characterImages, setCharacterImages] = useState<Record<string, string>>({});
  const [characterAvatars, setCharacterAvatars] = useState<Record<string, string>>({});
  const [playerImage, setPlayerImage] = useState<string | null>(null);
  const [playerAvatar, setPlayerAvatar] = useState<string | null>(null);
  const [gmImage, setGmImage] = useState<string | null>(null);
  // 三份載入各自的世代號：換桌或重讀時舊的回應要丟掉，免得把上一桌的圖補進新桌。
  // 用世代號而非 effect 的 stale 旗標，是因為這三支也給命令式重載（存完圖、匯完世界書）呼叫。
  const imagesLoad = useRef(0);
  const gmLoad = useRef(0);
  const playerLoad = useRef(0);

  const loadCharacterImages = useCallback(async (worldId: string, ids: string[]) => {
    const mine = ++imagesLoad.current;
    const entries = await Promise.all(
      ids.map(async (id) => {
        const [image, avatar] = await Promise.all([
          invoke<string | null>("read_character_image", { worldId, characterId: id }).catch(() => null),
          invoke<string | null>("read_character_avatar", { worldId, characterId: id }).catch(() => null),
        ]);
        return [id, image, avatar] as const;
      }),
    );
    if (mine !== imagesLoad.current) return;
    setCharacterImages(
      Object.fromEntries(
        entries.filter(([, image]) => image !== null).map(([id, image]) => [id, `data:image/png;base64,${image}`]),
      ),
    );
    setCharacterAvatars(
      Object.fromEntries(
        entries.filter(([, , avatar]) => avatar !== null).map(([id, , avatar]) => [id, `data:image/png;base64,${avatar}`]),
      ),
    );
  }, []);

  const loadGmImage = useCallback(async (worldId: string) => {
    const mine = ++gmLoad.current;
    const image = await invoke<string | null>("read_gm_image", { worldId }).catch(() => null);
    if (mine !== gmLoad.current) return;
    setGmImage(image ? `data:image/png;base64,${image}` : null);
  }, []);

  const loadPlayerCard = useCallback(async (worldId: string, playerCardId: string | null) => {
    const mine = ++playerLoad.current;
    if (!playerCardId) {
      setPlayerCard(null);
      setPlayerImage(null);
      setPlayerAvatar(null);
      return;
    }
    try {
      const [card, image, avatar] = await Promise.all([
        invoke<CharacterCard>("read_character", { worldId, characterId: playerCardId }),
        invoke<string | null>("read_character_image", { worldId, characterId: playerCardId }).catch(() => null),
        invoke<string | null>("read_character_avatar", { worldId, characterId: playerCardId }).catch(() => null),
      ]);
      if (mine !== playerLoad.current) return;
      setPlayerCard(card);
      setPlayerImage(image ? `data:image/png;base64,${image}` : null);
      setPlayerAvatar(avatar ? `data:image/png;base64,${avatar}` : null);
    } catch {
      if (mine !== playerLoad.current) return;
      setPlayerCard(null);
      setPlayerImage(null);
      setPlayerAvatar(null);
    }
  }, []);

  const active = useMemo(
    () => characters.filter((character) => !isCharacterHidden(character, sceneAppearances)),
    [characters, sceneAppearances],
  );
  const archived = useMemo(
    () => characters.filter((character) => isCharacterHidden(character, sceneAppearances)),
    [characters, sceneAppearances],
  );
  const metaOf = useCallback((id: string) => characters.find((c) => c.id === id), [characters]);

  // 圖只跟「有哪些角色」有關（存成 Record，不看順序）：排序後當 key，側欄拖曳排序就不會重讀圖
  const castIdsKey = useMemo(() => characters.map((c) => c.id).sort().join("\n"), [characters]);

  // 換桌的次要資源延後到 effect 載入，讓 enterTable 的 hydrate 一路同步、不被 await 切斷；
  // 上一桌的快取已由 hydrate 同步清掉，這裡只負責補上這桌的。
  useEffect(() => {
    if (!worldId) return;
    void loadCharacterImages(worldId, castIdsKey === "" ? [] : castIdsKey.split("\n"));
  }, [worldId, castIdsKey, loadCharacterImages]);

  useEffect(() => {
    if (!worldId) return;
    void loadGmImage(worldId);
  }, [worldId, loadGmImage]);

  useEffect(() => {
    if (!worldId) return;
    void loadPlayerCard(worldId, playerCardId);
  }, [worldId, playerCardId, loadPlayerCard]);

  const hydrate = useCallback((cast: CharacterMeta[], appearances: Set<string>, playerCardId: string | null) => {
    // 上一桌還在飛的三份載入一律作廢，免得回應晚到、把舊桌的圖補進新桌
    imagesLoad.current += 1;
    gmLoad.current += 1;
    playerLoad.current += 1;
    setCharacters(cast);
    setSceneAppearances(appearances);
    setPlayerCardId(playerCardId);
    // 圖與玩家卡同步清空：不清的話新名單已經上畫面、圖還是上一桌的，換桌瞬間會閃錯圖
    setCharacterImages({});
    setCharacterAvatars({});
    setPlayerImage(null);
    setPlayerAvatar(null);
    setGmImage(null);
    setPlayerCard(null);
  }, []);

  const refresh = useCallback(
    async (overrideWorldId?: string) => {
      const id = overrideWorldId ?? worldId;
      const cast = await invoke<CharacterMeta[]>("list_characters", { worldId: id });
      setCharacters(cast);
      await loadCharacterImages(id, cast.map((c) => c.id));
      return cast;
    },
    [worldId, loadCharacterImages],
  );

  const reloadImages = useCallback(
    async () => loadCharacterImages(worldId, characters.map((c) => c.id)),
    [worldId, characters, loadCharacterImages],
  );

  const reloadGmImage = useCallback(
    async (overrideWorldId?: string) => loadGmImage(overrideWorldId ?? worldId),
    [worldId, loadGmImage],
  );

  const reloadPlayer = useCallback(
    async (nextPlayerCardId: string | null) => {
      setPlayerCardId(nextPlayerCardId);
      await loadPlayerCard(worldId, nextPlayerCardId);
    },
    [worldId, loadPlayerCard],
  );

  const onArrived = useCallback((ids: string[]) => {
    setSceneAppearances((previous) => new Set([...previous, ...ids]));
  }, []);

  // 側欄拖曳排序：先樂觀套用，寫檔失敗才回捲
  // 用 archived（非 characters.filter(archived)）補回其餘卡片：
  // 這幕沒出場的 auto_hidden 卡不在 archived 旗標裡，漏掉會在拖曳當下從 state 消失
  const reorder = useCallback(
    async (ordered: CharacterMeta[]) => {
      onError("");
      const previous = characters;
      setCharacters([...ordered, ...archived]);
      try {
        await invoke("reorder_characters", {
          worldId,
          ids: ordered.map((character) => character.id),
        });
      } catch (reason) {
        setCharacters(previous);
        onError(String(reason));
      }
    },
    [characters, archived, worldId, onError],
  );

  const restore = useCallback(
    async (id: string) => {
      onError("");
      try {
        await invoke("set_character_archived", { worldId, characterId: id, archived: false });
        await refresh();
      } catch (reason) {
        onError(String(reason));
      }
    },
    [worldId, onError, refresh],
  );

  // 隱藏區裡 auto_hidden 卡的「拉回」：解除自動隱藏（不是解除封存），下次換幕結算才會重新判定
  const restoreAutoHidden = useCallback(
    async (id: string) => {
      onError("");
      try {
        await invoke("set_character_auto_hidden", { worldId, characterId: id, autoHidden: false });
        await refresh();
      } catch (reason) {
        onError(String(reason));
      }
    },
    [worldId, onError, refresh],
  );

  const noteRemoved = useCallback(
    async (removedId: string, speaker: string) => {
      const cast = await refresh();
      if (speaker !== removedId) return null;
      return cast.find((character) => !isCharacterHidden(character, sceneAppearances))?.id ?? "";
    },
    [refresh, sceneAppearances],
  );

  // 隱藏區與角色卡編輯畫面共用同一條刪除路徑（確認框＋刪檔），善後由呼叫端接手
  const remove = useCallback(
    async (id: string) => {
      onError("");
      try {
        const name = metaOf(id)?.name ?? id;
        const accepted = await confirm(t("deleteCharacterConfirm", { name }), {
          title: t("deleteCharacterTitle"),
          kind: "warning",
        });
        if (!accepted) return false;
        await invoke("delete_character", { worldId, characterId: id });
        return true;
      } catch (reason) {
        onError(String(reason));
        return false;
      }
    },
    [metaOf, worldId, onError],
  );

  const removePlayer = useCallback(
    async (id: string) => {
      onError("");
      try {
        const accepted = await confirm(t("deleteCharacterConfirm", { name: playerCard?.name ?? id }), {
          title: t("deleteCharacterTitle"),
          kind: "warning",
        });
        if (!accepted) return false;
        await invoke("delete_character", { worldId, characterId: id });
        const state = await invoke<WorldState>("read_state", { worldId });
        await invoke("write_state", { worldId, state: { ...state, player_card_id: null } });
        setPlayerCardId(null);
        setPlayerCard(null);
        setPlayerImage(null);
        setPlayerAvatar(null);
        return true;
      } catch (reason) {
        onError(String(reason));
        return false;
      }
    },
    [playerCard, worldId, onError],
  );

  return useMemo(
    () => ({
      list: characters,
      active,
      archived,
      images: characterImages,
      avatars: characterAvatars,
      player: playerCard,
      playerImage,
      playerAvatar,
      gmImage,
      metaOf,
      hydrate,
      refresh,
      reloadImages,
      reloadGmImage,
      reloadPlayer,
      onArrived,
      reorder,
      restore,
      restoreAutoHidden,
      noteRemoved,
      remove,
      removePlayer,
    }),
    [
      characters,
      active,
      archived,
      characterImages,
      characterAvatars,
      playerCard,
      playerImage,
      playerAvatar,
      gmImage,
      metaOf,
      hydrate,
      refresh,
      reloadImages,
      reloadGmImage,
      reloadPlayer,
      onArrived,
      reorder,
      restore,
      restoreAutoHidden,
      noteRemoved,
      remove,
      removePlayer,
    ],
  );
}
