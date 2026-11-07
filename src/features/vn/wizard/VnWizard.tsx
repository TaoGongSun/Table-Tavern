// VN 開桌精靈（plans/vn-setup-wizard.md §2）：一個對話窗、三格步驟條「點子 → 故事 → 美術」，可按上一步。
// ① 點子與偏好（或改用角色卡：交給匯入流程開 VN 桌，匯完以該桌從 ② 重開）→ ② 可改大綱、按「建立」展開成桌
// 並接著寫劇本聖經 → ③ 美術準備 → 開始遊戲＝進桌。①↔② 互退草稿不丟（草稿在本層）；建立後 ②↔③ 互退、① 變灰。
// ②③ 一經掛載就隱藏保留（劇本草稿、候選選擇不丟，返回不重生）。設定階段記在 vn.json，中途關掉大廳標「設定未完成」。
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { t } from "../../../i18n";
import type { VnSetupStage } from "../../../shared/contracts/backend-contracts";
import { Dialog, SwapLabel } from "../../../shared/ui/Dialog";
import { GenesisError, OutlinePreview } from "../../lobby/OutlinePreview";
import { useTableGenesis } from "../../lobby/useTableGenesis";
import { ArtStep, type ArtProgress } from "./ArtStep";
import { IdeaStep } from "./IdeaStep";
import { BuildProgress, StoryBuild, type BuildState } from "./StoryBuild";
import { EMPTY_DRAFT, genreLabels, toPreferences, type PreferenceDraft } from "./vn-preferences";
import "./vn-wizard.css";

/** worldId＝null 從 ① 開始；有值＝桌已建好（匯入來的、或大廳點了設定未完成的桌），從 step 起 */
export interface VnWizardStart {
  worldId: string | null;
  /** 匯入路由框直接進 ② 的沒有 ①，照預設鎖 */
  spoilerLock?: boolean;
  /** 桌已在時從哪一步起：VN 卡建的桌（聖經、角色已在卡上）與停在美術的桌從 art 起 */
  step?: VnSetupStage;
}

type Step = "idea" | "story" | "art";

const STEPS = [
  ["idea", "vnStepIdea"],
  ["story", "vnStepStory"],
  ["art", "vnStepArt"],
] as const;

export function VnWizard({
  start,
  transport,
  onCreated,
  onStage,
  onImportCard,
  onFinish,
  onClose,
}: {
  start: VnWizardStart;
  transport?: string;
  /** ② 展開建好桌：大廳清單重讀（不等生圖） */
  onCreated: (worldId: string) => Promise<void>;
  /** 設定階段寫進 vn.json 之後：大廳清單重讀（桌卡標記與重進路由跟著最新階段） */
  onStage?: () => void;
  /** ①「用我的角色卡」選了檔：開 VN 桌並匯入，匯完以該桌從 ② 重開精靈（帶 ① 的劇透鎖選擇） */
  onImportCard: (file: File, spoilerLock: boolean) => void;
  /** 按開始遊戲：進那張桌 */
  onFinish: (worldId: string) => void;
  onClose: () => void;
}) {
  const genesis = useTableGenesis("vn");
  // createTable 閉包看的是當下那次渲染的大綱；一鍵在 await 之後要先刷進主角名再拿最新的那份
  const genesisRef = useRef(genesis);
  genesisRef.current = genesis;
  const [auto, setAuto] = useState(false);
  const autoRef = useRef(false);
  autoRef.current = auto;
  const [draft, setDraft] = useState<PreferenceDraft>(EMPTY_DRAFT);
  const [spoilerLock, setSpoilerLock] = useState(start.spoilerLock ?? true);
  const [worldId, setWorldId] = useState(start.worldId);
  const [step, setStep] = useState<Step>(start.worldId === null ? "idea" : (start.step ?? "story"));
  // 建桌後 ②③ 第一次顯示才掛載（重開到 ③ 的桌不會偷偷補寫聖經），掛了就隱藏保留；③ 在建桌當下就掛（候選排隊）
  const [visitedStory, setVisitedStory] = useState(step === "story");
  const [visitedArt, setVisitedArt] = useState(step === "art");
  const [build, setBuild] = useState<BuildState>("running");
  const [art, setArt] = useState<ArtProgress>({ ready: 0, total: 0 });
  const [starting, setStarting] = useState(false);
  const finishArt = useRef<(() => Promise<boolean>) | null>(null);
  const flushStory = useRef<(() => Promise<boolean>) | null>(null);
  const [storyDirty, setStoryDirty] = useState(false);
  const onStoryDirty = useCallback(setStoryDirty, []);
  const onArt = useCallback(setArt, []);
  const beginRef = useRef<() => Promise<void>>(async () => {});
  // 聖經完成且還在一鍵：直接進標題；寫失敗就退出一鍵，停在 ② 既有的重試畫面
  const onBuild = useCallback((next: BuildState) => {
    setBuild(next);
    if (!autoRef.current) return;
    if (next === "failed") setAuto(false);
    else if (next === "done") void beginRef.current();
  }, []);

  // 桌在了就記下停在哪一步（重開 app 也回得來）
  useEffect(() => {
    if (worldId === null || step === "idea") return;
    if (step === "art") setVisitedArt(true);
    else setVisitedStory(true);
    invoke("vn_setup_set", { worldId, stage: step, spoilerLock })
      .then(() => onStage?.())
      .catch(() => {});
  }, [worldId, step]);

  /** 劇本規格層有未存改動就先存；失敗回 ② 讓玩家看到訊息，草稿留著 */
  async function saveStory(): Promise<boolean> {
    if (!storyDirty) return true;
    const ok = (await flushStory.current?.()) ?? true;
    if (!ok) setStep("story");
    return ok;
  }

  const labels = genreLabels(draft);
  const busy = genesis.busy !== null || starting;
  const setName = (name: string) => {
    setDraft((current) => ({ ...current, name }));
    genesis.setOutline((current) => current && { ...current, player: { name, tagline: current.player?.tagline ?? "" } });
  };

  async function generate() {
    if (!(await genesis.generateOutline(labels, toPreferences(draft)))) return;
    // 主角名以玩家填的為準（AI 提的身分可在 ② 改）
    genesis.setOutline((current) =>
      current && { ...current, player: { name: draft.name, tagline: current.player?.tagline ?? "" } },
    );
    setStep("story");
  }

  async function create() {
    const id = await genesis.createTable(labels, {
      preferences: toPreferences(draft, genesis.outline?.player?.tagline ?? ""),
      spoilerLock,
    });
    if (id === null) return;
    setWorldId(id);
    void onCreated(id);
  }

  /** 不審任何一步。大綱或建桌失敗就退出一鍵，留下已生成的東西走逐步畫面 */
  async function startAuto() {
    setAuto(true);
    const prefs = toPreferences(draft);
    const name = draft.name;
    if (!(await genesisRef.current.generateOutline(labels, prefs))) {
      setAuto(false);
      return;
    }
    flushSync(() => {
      genesisRef.current.setOutline((current) =>
        current && { ...current, player: { name, tagline: current.player?.tagline ?? "" } },
      );
    });
    const id = await genesisRef.current.createTable(labels, {
      preferences: toPreferences(draft, genesisRef.current.outline?.player?.tagline ?? ""),
      spoilerLock,
    });
    if (id === null) {
      setAuto(false);
      setStep("story");
      return;
    }
    setWorldId(id);
    setStep("story");
    setVisitedStory(true);
    void onCreated(id);
    invoke("vn_backgrounds_fill", { worldId: id, scene: null }).catch(() => {});
  }

  async function begin() {
    if (worldId === null) return;
    setStarting(true);
    try {
      if (!(await saveStory())) {
        setStarting(false);
        setAuto(false);
        return;
      }
      // 定稿裝不上：留在 ③，失敗顯示在該列，再按開始＝重試
      if ((await finishArt.current?.()) === false) {
        setStarting(false);
        if (autoRef.current) {
          setAuto(false);
          setStep("art");
        }
        return;
      }
      await invoke("vn_setup_set", { worldId, stage: null, spoilerLock: null });
      // 開演：開始在背景補表情（播放器可能早已掛著，不能只靠它掛載時排）
      invoke("vn_sprites_fill", { worldId }).catch(() => {});
      onFinish(worldId);
    } catch {
      setStarting(false);
      setAuto(false);
    }
  }
  beginRef.current = begin;

  const back = step === "art" ? "story" : step === "story" && worldId === null ? "idea" : null;
  // 桌還沒建、大綱在手：關掉會丟掉大綱，先問一句
  async function close() {
    if (worldId === null && genesis.outline) {
      const ok = await confirm(t("vnCloseDropOutline"), {
        kind: "warning",
        okLabel: t("closeBtn"),
        cancelLabel: t("dialogCancel"),
      });
      if (!ok) return;
    }
    if (await saveStory()) onClose();
  }
  const ready = art.total > 0 && art.ready < art.total;
  const autoPhase: "outline" | "world" | "bible" | "done" =
    worldId === null ? (genesis.busy === "expand" ? "world" : "outline") : build === "done" ? "done" : "bible";
  const footer =
    step === "idea" ? (
      genesis.outline && (
        <button type="button" className="btn btn-primary" disabled={busy} onClick={() => setStep("story")}>
          {t("vnNext")}
        </button>
      )
    ) : step === "story" && worldId === null ? (
      <>
        <label className="inline vn-spoiler-toggle">
          <input
            type="checkbox"
            checked={spoilerLock}
            disabled={busy}
            onChange={(event) => setSpoilerLock(event.currentTarget.checked)}
          />
          {t("vnSpoilerCheckbox")}
        </label>
        <button
          type="button"
          className="btn btn-primary"
          disabled={busy || !genesis.canCreate}
          onClick={() => void create()}
        >
          <SwapLabel labels={[t("genCreateBtn"), t("genExpanding")]} current={genesis.busy === "expand" ? 1 : 0} />
        </button>
      </>
    ) : step === "story" ? (
      <button
        type="button"
        className="btn btn-primary"
        disabled={build === "running"}
        onClick={() => void saveStory().then((ok) => ok && setStep("art"))}
      >
        {t("vnNext")}
      </button>
    ) : (
      <button type="button" className="btn btn-primary" disabled={starting} onClick={() => void begin()}>
        {ready ? t("vnStartPreparing", { n: art.ready, total: art.total }) : t("vnStartGame")}
      </button>
    );

  return (
    <Dialog
      title={t("vnWizardTitle")}
      size="l"
      className={`gen-table-dialog vn-wizard${genesis.busy === "expand" ? " vn-wizard-building" : ""}`}
      onDismiss={busy ? undefined : () => void close()}
      closeButton
      toolbar={
        <ol className="vn-stepbar">
          {STEPS.map(([id, key], index) => (
            <li
              key={id}
              aria-current={step === id ? "step" : undefined}
              data-disabled={id === "idea" && worldId !== null ? "" : undefined}
            >
              {/* 已走過、可退回的那格可點（同上一步）；一鍵進行中整條不可點 */}
              {!auto && id === back ? (
                <button type="button" disabled={busy} onClick={() => setStep(back)}>
                  {index + 1}. {t(key)}
                </button>
              ) : (
                `${index + 1}. ${t(key)}`
              )}
            </li>
          ))}
        </ol>
      }
      start={
        !auto &&
        back && (
          <button type="button" className="btn" disabled={busy} onClick={() => setStep(back)}>
            {t("vnPrevStep")}
          </button>
        )
      }
      end={auto ? undefined : footer}
    >
      {auto && <BuildProgress outline phase={autoPhase} />}
      {!auto && step === "idea" && (
        <IdeaStep
          genesis={genesis}
          draft={draft}
          setDraft={setDraft}
          onGenerate={() => void generate()}
          onStart={() => void startAuto()}
          spoilerLock={spoilerLock}
          setSpoilerLock={setSpoilerLock}
          onImportCard={onImportCard}
        />
      )}
      {!auto && step === "story" && worldId === null && (
        <>
          {genesis.busy === "expand" && <BuildProgress phase="world" />}
          <OutlinePreview
            genesis={genesis}
            genreLabels={labels}
            player={
              genesis.outline && (
                <div className="vn-player-row">
                  <h3>{t("vnProtagonist")}</h3>
                  <input
                    value={draft.name}
                    placeholder={t("vnYourNamePlaceholder")}
                    aria-label={t("vnYourName")}
                    disabled={busy}
                    onChange={(event) => setName(event.currentTarget.value)}
                  />
                  <textarea
                    rows={2}
                    value={genesis.outline.player?.tagline ?? ""}
                    aria-label={t("vnProtagonistRole")}
                    placeholder={t("vnProtagonistRole")}
                    disabled={busy}
                    onChange={(event) => {
                      const tagline = event.currentTarget.value;
                      genesis.setOutline((current) =>
                        current && { ...current, player: { name: current.player?.name ?? "", tagline } },
                      );
                    }}
                  />
                </div>
              )
            }
          />
        </>
      )}
      {!auto && (step === "idea" || (step === "story" && worldId === null)) && (
        <GenesisError genesis={genesis} transport={transport} />
      )}
      {worldId !== null && (
        <>
          {visitedStory && (
            <div hidden={auto || step !== "story"}>
              <StoryBuild
                worldId={worldId}
                spoilerLock={spoilerLock}
                onState={onBuild}
                onDirtyChange={onStoryDirty}
                flushRef={flushStory}
              />
            </div>
          )}
          <div hidden={step !== "art"}>
            <ArtStep worldId={worldId} active={visitedArt} onProgress={onArt} finishRef={finishArt} />
          </div>
        </>
      )}
    </Dialog>
  );
}
