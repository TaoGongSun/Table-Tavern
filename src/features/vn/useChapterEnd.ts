// VN 章結束流程（vn-mode 包 5 §2.3）：章末字卡 → 換幕（章結束才加章）→ 重進本桌 → 自動開下一章首拍。
// 換幕分階段：沒提交＝字卡顯示原因、重試才再換；已提交但重進失敗＝只重試重進，絕不再換一次幕。
// 首拍要等播放器拿到新幕（scene 變了）才開：重進後同一輪 render 還沒換手，舊閉包的 GM 推進會落到舊幕。
import { useCallback, useRef, useState } from "react";
import type { VnAdvance } from "../play/scene/useSceneActions";

export interface ChapterCard {
  /** advancing＝換幕中；failed＝沒提交；reenter＝已換幕、重進失敗；opening＝等下一章第一格 */
  phase: "advancing" | "failed" | "reenter" | "opening";
  /** 章結束入口（GM `[chapter]`、「下一章」）；容量換幕為 false，不播「第 N 章 完」 */
  chapterEnd: boolean;
  /** 結束的章號 */
  chapter: number;
  error: string | null;
  /** 流程開始時的幕號：播放器的 scene 換成別的才開首拍 */
  fromScene: number;
  /** opening 且首拍還沒開 */
  beat: boolean;
}

export function useChapterEnd({
  advance,
  reenter,
  openBeat,
  runLocked,
}: {
  advance: (chapterEnd: boolean) => Promise<VnAdvance>;
  reenter: () => Promise<boolean>;
  /** 重進成功後開下一章首拍（GM 推進） */
  openBeat: () => Promise<void>;
  /** 播放器的同步動作鎖：整段流程佔著，選項、繼續、換一組都按不了 */
  runLocked: (work: () => Promise<void>) => Promise<void>;
}) {
  const [card, setCard] = useState<ChapterCard | null>(null);
  const cardRef = useRef(card);
  cardRef.current = card;

  const open = useCallback(() => setCard((c) => c && { ...c, phase: "opening", error: null, beat: true }), []);

  /** 播放器換到新幕後開下一章首拍（由播放器在 scene 變了時呼叫） */
  const beginBeat = useCallback(
    () =>
      runLocked(async () => {
        setCard((c) => c && { ...c, beat: false });
        try {
          await openBeat();
        } finally {
          setCard(null);
        }
      }),
    [runLocked, openBeat],
  );

  const step = useCallback(
    async (chapterEnd: boolean, chapter: number, fromScene: number) => {
      setCard({ phase: "advancing", chapterEnd, chapter, error: null, fromScene, beat: false });
      const result = await advance(chapterEnd);
      if (result.stage === "failed") {
        setCard((c) => c && { ...c, phase: "failed", error: result.message });
      } else if (result.stage === "committed") {
        setCard((c) => c && { ...c, phase: "reenter", error: null });
      } else {
        open();
      }
    },
    [advance, open],
  );

  /** 開始章結束流程；已有字卡在跑就不理 */
  const start = useCallback(
    (chapterEnd: boolean, chapter: number, fromScene: number) => {
      if (cardRef.current) return Promise.resolve();
      return runLocked(() => step(chapterEnd, chapter, fromScene));
    },
    [runLocked, step],
  );

  /** 字卡上的重試：沒提交就重跑換幕，已提交就只重進 */
  const retry = useCallback(() => {
    const now = cardRef.current;
    if (!now) return Promise.resolve();
    return runLocked(async () => {
      if (now.phase === "failed") return step(now.chapterEnd, now.chapter, now.fromScene);
      if (now.phase !== "reenter") return;
      setCard({ ...now, phase: "advancing" });
      if (await reenter()) open();
      else setCard({ ...now, phase: "reenter" });
    });
  }, [runLocked, step, reenter, open]);

  /** 下一章第一格出來就收起字卡（首拍還在背景接力） */
  const close = useCallback(() => setCard(null), []);

  return { card, start, retry, close, beginBeat };
}
