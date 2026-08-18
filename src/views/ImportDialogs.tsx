// 匯入流程跳出來的三個框：匯入身分框、第二張卡路由框、匯完的開場白選擇面板。
// 整支是受控元件——身分／路由／開場白清單、展開的那一則與翻譯狀態都由 imports controller 擁有，
// 貼出開場白仍由 App 協調 chat 與 imports（見 postOpening／postTranslatedOpening），這裡只畫與回報。
import { t } from "../i18n";
import {
  OpeningTranslationState,
  PendingImportChoice,
  PendingImportRoute,
  Tier,
  TierModel,
} from "../controllers/useImportController";
import { StoryText } from "./atoms";

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
  onPostOpening: (text: string) => void;
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
  onTranslateAndPost,
  onRetranslate,
}: ImportDialogsProps) {
  return (
    <>
      {/* 匯入身分框：有名字的卡一律問。直說偵測到哪一種，該身分當主按鈕，另一邊只警告可能玩不動 */}
      {choice !== null && (
        <div className="modal-overlay" onClick={() => onAnswerChoice("cancel")}>
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label={t(choice.booksFirst ? "importChoiceBookTitle" : "importChoiceCharacterTitle")}
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t(choice.booksFirst ? "importChoiceBookTitle" : "importChoiceCharacterTitle")}</h2>
            <p>{t(choice.booksFirst ? "importChoiceBookBody" : "importChoiceCharacterBody")}</p>
            <div className="ai-gen-footer">
              <button type="button" onClick={() => onAnswerChoice("cancel")}>
                {t("importChoiceCancel")}
              </button>
              {choice.booksFirst ? (
                <>
                  <button type="button" onClick={() => onAnswerChoice("character")}>
                    {t("importChoiceCharacter")}
                  </button>
                  <button type="button" className="ai-gen-submit" onClick={() => onAnswerChoice("worldbook")}>
                    {t("importChoiceWorldbook")}
                  </button>
                </>
              ) : (
                <>
                  <button type="button" onClick={() => onAnswerChoice("worldbook")}>
                    {t("importChoiceWorldbook")}
                  </button>
                  <button type="button" className="ai-gen-submit" onClick={() => onAnswerChoice("character")}>
                    {t("importChoiceCharacter")}
                  </button>
                </>
              )}
            </div>
          </div>
        </div>
      )}

      {/* 第二張卡路由框：桌上已有匯入紀錄才會跳出來。三個選項都給，開新桌是主按鈕；
          第二本世界書換標題與文案（會合成一本），中間那顆改叫「仍要匯入」 */}
      {route !== null && (
        <div className="modal-overlay" onClick={() => onAnswerRoute("cancel")}>
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label={t(route.route === "merge_worldbook" ? "importRouteMergeTitle" : "importRouteAskTitle")}
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t(route.route === "merge_worldbook" ? "importRouteMergeTitle" : "importRouteAskTitle")}</h2>
            <p>{t(route.route === "merge_worldbook" ? "importRouteMergeBody" : "importRouteAskBody")}</p>
            <div className="ai-gen-footer">
              <button type="button" onClick={() => onAnswerRoute("cancel")}>
                {t("importChoiceCancel")}
              </button>
              <button type="button" onClick={() => onAnswerRoute("this_table")}>
                {t(route.route === "merge_worldbook" ? "importRouteMergeAnyway" : "importRouteThisTable")}
              </button>
              <button
                type="button"
                className="ai-gen-submit"
                onClick={() => onAnswerRoute("new_table")}
                disabled={busy}
              >
                {t("importRouteNewTable")}
              </button>
            </div>
          </div>
        </div>
      )}

      {openings !== null && (
        <div className="modal-overlay" onClick={() => onCloseOpenings()}>
          <div
            className="modal opening-choice-modal"
            role="dialog"
            aria-modal="true"
            aria-label={t("openingChoiceTitle")}
            onClick={(event) => event.stopPropagation()}
          >
            <div className="modal-header">
              <strong>{t("openingChoiceTitle")}</strong>
              <button type="button" className="modal-close" aria-label={t("closeBtn")} onClick={() => onCloseOpenings()}>×</button>
            </div>
            {/* 動作鈕置頂（專案慣例）：全部翻譯放標題正下方，不必展開任何一則就能先按。
                檔位挑選器就長在鈕旁邊——玩家不必翻說明也知道翻譯用的是哪個模型，
                翻不出來（模型拒譯）時往上調一檔再重新翻譯。只影響這次視窗，不寫回設定。 */}
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
                className="ai-gen-btn"
                title={t("openingTranslateHint")}
                disabled={translateAllBusy}
                onClick={() => onTranslateAll()}
              >
                {translateAllBusy
                  ? t("openingTranslateAllProgress", {
                      done: openings.filter((_, index) => translationState[index] === "done" || translationState[index] === "error")
                        .length,
                      total: openings.length,
                    })
                  : `✨ ${t("openingTranslateAllBtn")}`}
              </button>
            </div>
            <p>{t("openingLineAsk")}</p>
            <div className="opening-choice-list">
              {openings.map((opening, index) => {
                // 點列只展開全文，貼出的鈕在框外底部——開場白動輒上千字，按鈕若跟在全文後面
                // 得整段捲到底才按得到，而滿是標記的開場白根本沒必要逐字看完
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
                      {transState === "translating" && <span className="opening-trans-status">{t("openingTranslating")}</span>}
                      {transState === "error" && (
                        <span className="opening-trans-status opening-trans-error" title={t("openingTranslateFailed")}>
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
            <div className="ai-gen-footer">
              {expanded !== null && openings[expanded] !== undefined && (
                <>
                  <button
                    type="button"
                    className="footer-lead"
                    onClick={() => onPostOpening(translations[expanded] ?? openings[expanded])}
                  >
                    {t("openingLineOk")}
                  </button>
                  {/* 翻過（成功或失敗）才出現：模型翻不出來或翻壞了，調高上方檔位再打一次。
                      同樣檔位連按不擋——同一個模型重跑本來就可能給出不一樣的結果 */}
                  {translationState[expanded] !== undefined && (
                    <button
                      type="button"
                      className="ai-gen-btn"
                      title={t("openingTranslateHint")}
                      disabled={translationState[expanded] === "translating"}
                      onClick={() => onRetranslate(expanded)}
                    >
                      {t("openingRetranslateBtn")}
                    </button>
                  )}
                  <button
                    type="button"
                    className="ai-gen-btn"
                    title={t("openingTranslateHint")}
                    disabled={translationState[expanded] === "translating"}
                    onClick={() => onTranslateAndPost(expanded)}
                  >
                    {translationState[expanded] === "translating" ? t("openingTranslating") : `✨ ${t("openingTranslatePostBtn")}`}
                  </button>
                </>
              )}
              <button type="button" onClick={() => onCloseOpenings()}>{t("openingLineCancel")}</button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
