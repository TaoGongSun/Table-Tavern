// 匯入流程跳出來的三個框：匯入身分框、第二張卡路由框、匯完的開場白選擇面板。
// 整支是受控元件——身分／路由／開場白清單、展開的那一則與翻譯狀態都由 imports controller 擁有，
// 貼出開場白仍由 App 協調 chat 與 imports（見 postOpening／postTranslatedOpening），這裡只畫與回報。
import { t } from "../../i18n";
import {
  OpeningTranslationState,
  PendingImportChoice,
  PendingImportRoute,
  Tier,
  TierModel,
} from "./useImportController";
import { Dialog, SwapLabel } from "../../shared/ui/Dialog";
import { StoryText } from "../../shared/ui/atoms";

/** 檔位選項的字：「低 · claude-haiku-4-5」。同一家的不同世代對同樣內容的容忍度不一樣，
    只寫「sonnet」分不出 4.6 與 5，所以顯示實際 id。model 為 null＝走 CLI 預設模型。 */
function tierLabel(tier: Tier, models: TierModel[]) {
  const name = t(tier === "fast" ? "tierFast" : tier === "balanced" ? "tierBalanced" : "tierBest");
  const found = models.find((entry) => entry.tier === tier);
  if (!found) return name;
  const model =
    found.model ?? t("openingTierCliDefault") + (found.effort ? ` · ${found.effort}` : "");
  return `${name} · ${model}`;
}

function openingPreview(text: string) {
  const preview = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .slice(0, 2)
    .join(" ");
  return preview.length > 120 ? `${preview.slice(0, 119)}…` : preview;
}

interface ImportDialogsProps {
  /** 生成中：路由框的「開新桌並匯入」會切桌，跟其他桌次操作一樣停用 */
  busy: boolean;
  /** 匯入身分框：null＝沒開 */
  choice: PendingImportChoice | null;
  onAnswerChoice: (answer: "character" | "worldbook" | "cancel") => void;
  /** 第二張卡路由框：null＝沒開 */
  route: PendingImportRoute | null;
  onAnswerRoute: (answer: "this_table" | "new_table" | "cancel") => void;
  /** 開場白清單；null＝面板沒開 */
  openings: string[] | null;
  /** 面板裡展開的那一則（一次只展開一條）；null＝全部收著 */
  expanded: number | null;
  /** 逐則翻譯狀態 */
  translationState: OpeningTranslationState;
  /** 已收到的譯文：有值就顯示這個，沒有才顯示 openings 的原文 */
  translations: Record<number, string>;
  /** 「全部翻譯」跑著沒 */
  translateAllBusy: boolean;
  /** 這次視窗的翻譯檔位（不動全域設定） */
  tier: Tier;
  onSetTier: (tier: Tier) => void;
  /** 三檔各自實際會叫的模型，後端解析 */
  tierModels: TierModel[];
  onSetExpanded: (index: number | null) => void;
  onCloseOpenings: () => void;
  onTranslateAll: () => void;
  /** 貼出這一則（有譯文就是譯文） */
  /** index＝這則在開場白清單裡的序號（重新重構時從原卡取同一則當初始值依據） */
  /** translated＝貼的是翻譯版（後端正文用它，巨集副作用照原文） */
  onPostOpening: (text: string, index: number, translated: boolean) => void;
  /** 貼出進行中：兩顆貼出鈕停用，不重複送出 */
  postBusy: boolean;
  /** 正排在進行中的回合後面：貼出鈕就地換成等待提示 */
  postWaiting: boolean;
  /** 翻譯後貼出：沒翻過就先翻這一則 */
  onTranslateAndPost: (index: number) => void;
  /** 重新翻譯這一則：用原文重打，會再花一次額度 */
  onRetranslate: (index: number) => void;
}

export function ImportDialogs({
  busy,
  choice,
  onAnswerChoice,
  route,
  onAnswerRoute,
  openings,
  expanded,
  translationState,
  translations,
  translateAllBusy,
  tier,
  onSetTier,
  tierModels,
  onSetExpanded,
  onCloseOpenings,
  onTranslateAll,
  onPostOpening,
  postBusy,
  postWaiting,
  onTranslateAndPost,
  onRetranslate,
}: ImportDialogsProps) {
  return (
    <>
      {/* 匯入身分框：有名字的卡一律問。直說偵測到哪一種，該身分當主按鈕，另一邊只警告可能玩不動 */}
      {choice !== null && (
        <Dialog
          title={t(choice.booksFirst ? "importChoiceBookTitle" : "importChoiceCharacterTitle")}
          onDismiss={() => onAnswerChoice("cancel")}
          backdrop
          start={
            <>
              <button type="button" className="btn" onClick={() => onAnswerChoice("cancel")}>
                {t("importChoiceCancel")}
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => onAnswerChoice(choice.booksFirst ? "character" : "worldbook")}
              >
                {t(choice.booksFirst ? "importChoiceCharacter" : "importChoiceWorldbook")}
              </button>
            </>
          }
          end={
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => onAnswerChoice(choice.booksFirst ? "worldbook" : "character")}
            >
              {t(choice.booksFirst ? "importChoiceWorldbook" : "importChoiceCharacter")}
            </button>
          }
        >
          <p>{t(choice.booksFirst ? "importChoiceBookBody" : "importChoiceCharacterBody")}</p>
        </Dialog>
      )}

      {/* 第二張卡路由框：桌上已有匯入紀錄才會跳出來。三個選項都給，開新桌是主按鈕；
          第二本世界書換標題與文案（會合成一本），中間那顆改叫「仍要匯入」 */}
      {route !== null && (
        <Dialog
          title={t(
            route.route === "merge_worldbook" ? "importRouteMergeTitle" : "importRouteAskTitle",
          )}
          onDismiss={() => onAnswerRoute("cancel")}
          backdrop
          start={
            <>
              <button type="button" className="btn" onClick={() => onAnswerRoute("cancel")}>
                {t("importChoiceCancel")}
              </button>
              <button type="button" className="btn" onClick={() => onAnswerRoute("this_table")}>
                {t(
                  route.route === "merge_worldbook"
                    ? "importRouteMergeAnyway"
                    : "importRouteThisTable",
                )}
              </button>
            </>
          }
          end={
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => onAnswerRoute("new_table")}
              disabled={busy}
            >
              {t("importRouteNewTable")}
            </button>
          }
        >
          <p>
            {t(route.route === "merge_worldbook" ? "importRouteMergeBody" : "importRouteAskBody")}
          </p>
        </Dialog>
      )}

      {openings !== null && (
        <Dialog
          title={t("openingChoiceTitle")}
          size="l"
          className="opening-choice-dialog"
          onDismiss={() => onCloseOpenings()}
          backdrop
          closeButton
          // 動作鈕置頂（專案慣例）：全部翻譯放標題正下方，不必展開任何一則就能先按。
          // 檔位挑選器就長在鈕旁邊——玩家不必翻說明也知道翻譯用的是哪個模型，
          // 翻不出來（模型拒譯）時往上調一檔再重新翻譯。只影響這次視窗，不寫回設定。
          toolbar={
            <div className="opening-translate-all-row">
              <label className="opening-tier-pick">
                {t("openingTranslateTier")}
                <select
                  value={tier}
                  disabled={translateAllBusy}
                  onChange={(event) => onSetTier(event.target.value as Tier)}
                >
                  {(["fast", "balanced", "best"] as Tier[]).map((option) => (
                    <option key={option} value={option}>
                      {tierLabel(option, tierModels)}
                    </option>
                  ))}
                </select>
              </label>
              <button
                type="button"
                className="btn opening-translate-all"
                title={t("openingTranslateHint")}
                disabled={translateAllBusy}
                onClick={() => onTranslateAll()}
              >
                {translateAllBusy
                  ? t("openingTranslateAllProgress", {
                      done: openings.filter(
                        (_, index) =>
                          translationState[index] === "done" || translationState[index] === "error",
                      ).length,
                      total: openings.length,
                    })
                  : `✨ ${t("openingTranslateAllBtn")}`}
              </button>
            </div>
          }
          start={
            <button type="button" className="btn" onClick={() => onCloseOpenings()}>
              {t("openingLineCancel")}
            </button>
          }
          // 點列只展開全文，貼出的鈕在底部——開場白動輒上千字，按鈕若跟在全文後面
          // 得整段捲到底才按得到，而滿是標記的開場白根本沒必要逐字看完
          end={
            expanded !== null &&
            openings[expanded] !== undefined && (
              <>
                {/* 翻過（成功或失敗）才看得到，沒翻過也占著位子：翻譯前後右邊兩顆不位移。
                    模型翻不出來或翻壞了，調高上方檔位再打一次；同樣檔位連按不擋——
                    同一個模型重跑本來就可能給出不一樣的結果 */}
                <button
                  type="button"
                  className={
                    translationState[expanded] === undefined ? "btn dialog-slot-idle" : "btn"
                  }
                  title={t("openingTranslateHint")}
                  aria-hidden={translationState[expanded] === undefined}
                  disabled={
                    translationState[expanded] === undefined ||
                    translationState[expanded] === "translating"
                  }
                  onClick={() => onRetranslate(expanded)}
                >
                  {t("openingRetranslateBtn")}
                </button>
                <button
                  type="button"
                  className="btn"
                  title={t("openingTranslateHint")}
                  disabled={postBusy || translationState[expanded] === "translating"}
                  onClick={() => onTranslateAndPost(expanded)}
                >
                  <SwapLabel
                    labels={[`✨ ${t("openingTranslatePostBtn")}`, t("openingTranslating")]}
                    current={translationState[expanded] === "translating" ? 1 : 0}
                  />
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={postBusy}
                  onClick={() =>
                    onPostOpening(
                      translations[expanded] ?? openings[expanded],
                      expanded,
                      translations[expanded] !== undefined,
                    )
                  }
                >
                  <SwapLabel
                    labels={[t("openingLineOk"), t("turnQueuedWait")]}
                    current={postWaiting ? 1 : 0}
                  />
                </button>
              </>
            )
          }
        >
          <p>{t("openingLineAsk")}</p>
          <div className="opening-choice-list">
            {openings.map((opening, index) => {
              const isExpanded = expanded === index;
              const transState = translationState[index];
              // 譯文一到就取代畫面上的原文（玩家看不懂原文，留著沒意義）；
              // 原文仍在 openings 裡，重新翻譯拿它當輸入
              const shown = translations[index] ?? opening;
              return (
                <div className="opening-choice-item" key={index}>
                  <button
                    type="button"
                    className="opening-choice-head"
                    aria-expanded={isExpanded}
                    onClick={() => onSetExpanded(isExpanded ? null : index)}
                  >
                    <strong>{t("openingChoiceItem", { n: index + 1 })}</strong>
                    {transState === "translating" && (
                      <span className="opening-trans-status">{t("openingTranslating")}</span>
                    )}
                    {transState === "error" && (
                      <span
                        className="opening-trans-status opening-trans-error"
                        title={t("openingTranslateFailed")}
                      >
                        ⚠
                      </span>
                    )}
                    <span>{isExpanded ? "" : openingPreview(shown)}</span>
                  </button>
                  {isExpanded && (
                    <div className="opening-choice-full">
                      <StoryText text={shown} />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </Dialog>
      )}
    </>
  );
}
