import type { AppConfig } from "../backend-contracts";
import type { ImportController } from "../controllers/useImportController";
import { t } from "../i18n";
import { GenerateTableDialog } from "./GenerateTableDialog";
import { ImportDialogs } from "./ImportDialogs";
import { SettingsWindow } from "./SettingsWindow";

interface AppDialogsProps {
  genTableOpen: boolean;
  onCloseGenerateTable: () => void;
  onGeneratedTable: (worldId: string) => Promise<void>;
  settingsOpen: false | "appearance" | "ai";
  config: AppConfig;
  onConfigSaved: (config: AppConfig) => void;
  onSettingPreference: (key: string, value: unknown) => Promise<void>;
  sponsorUnlocked: boolean;
  onSponsorUnlocked: () => void;
  onCloseSettings: () => void;
  currentWorld: string;
  regenOpen: boolean;
  onAnswerRegen: (answer: "regen" | "keep" | "cancel") => Promise<void>;
  chatBusy: boolean;
  imports: ImportController;
  onPostOpening: (text: string) => Promise<void>;
  onTranslateAndPost: (index: number) => Promise<void>;
}

export function AppDialogs({
  genTableOpen,
  onCloseGenerateTable,
  onGeneratedTable,
  settingsOpen,
  config,
  onConfigSaved,
  onSettingPreference,
  sponsorUnlocked,
  onSponsorUnlocked,
  onCloseSettings,
  currentWorld,
  regenOpen,
  onAnswerRegen,
  chatBusy,
  imports,
  onPostOpening,
  onTranslateAndPost,
}: AppDialogsProps) {
  return (
    <>
      <GenerateTableDialog
        open={genTableOpen}
        onClose={onCloseGenerateTable}
        onCreated={onGeneratedTable}
      />

      {/* 設定視窗要蓋在主工作區與生桌對話框之上。 */}
      {settingsOpen !== false && (
        <SettingsWindow
          config={config}
          onSaved={onConfigSaved}
          onPreference={(key, value) => void onSettingPreference(key, value)}
          sponsorUnlocked={sponsorUnlocked}
          onSponsorUnlocked={onSponsorUnlocked}
          onClose={onCloseSettings}
          initialTab={settingsOpen}
          currentWorld={currentWorld}
        />
      )}

      {/* 換語言後的範例桌詢問疊在設定視窗之上。 */}
      {regenOpen && (
        <div className="modal-overlay" onClick={() => void onAnswerRegen("cancel")}>
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label={t("sampleRegenTitle")}
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t("sampleRegenTitle")}</h2>
            <p>{t("sampleRegenBody")}</p>
            <div className="ai-gen-footer">
              <button type="button" onClick={() => void onAnswerRegen("cancel")}>
                {t("sampleRegenCancel")}
              </button>
              <button type="button" onClick={() => void onAnswerRegen("keep")}>
                {t("sampleRegenKeep")}
              </button>
              <button type="button" onClick={() => void onAnswerRegen("regen")}>
                {t("sampleRegenConfirm")}
              </button>
            </div>
          </div>
        </div>
      )}

      <ImportDialogs
        busy={chatBusy}
        choice={imports.choice}
        onAnswerChoice={(answer) => void imports.answerChoice(answer)}
        route={imports.route}
        onAnswerRoute={(answer) => void imports.answerRoute(answer)}
        openings={imports.openings}
        expanded={imports.expanded}
        translationState={imports.transState}
        translations={imports.translations}
        translateAllBusy={imports.transAllBusy}
        tier={imports.transTier}
        onSetTier={imports.setTransTier}
        tierModels={imports.tierModels}
        onSetExpanded={imports.setExpanded}
        onCloseOpenings={imports.closeOpenings}
        onTranslateAll={() => void imports.translateAllOpenings()}
        onPostOpening={(text) => void onPostOpening(text)}
        onTranslateAndPost={(index) => void onTranslateAndPost(index)}
        onRetranslate={(index) => void imports.translateOpening(index, true)}
      />
    </>
  );
}
