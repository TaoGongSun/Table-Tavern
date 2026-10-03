import type { ReactNode } from "react";
import type { AppConfig } from "../shared/contracts/backend-contracts";
import type { ImportController } from "../features/import/useImportController";
import { t } from "../i18n";
import { Dialog } from "../shared/ui/Dialog";
import { GenerateTableDialog } from "../features/lobby/GenerateTableDialog";
import { ImportDialogs } from "../features/import/ImportDialogs";
import { SettingsWindow, type SettingsTab } from "../features/settings/SettingsWindow";

interface AppDialogsProps {
  genTableOpen: boolean;
  onCloseGenerateTable: () => void;
  onGeneratedTable: (worldId: string) => Promise<void>;
  settingsOpen: false | SettingsTab;
  settingsRequestKey: number;
  versionTab: ReactNode;
  /** 啟動時的含格式轉換更新對話框。 */
  updateDialog: ReactNode;
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
  onPostOpening: (text: string, index: number) => Promise<void>;
  /** 貼出進行中（含排在回合後面等待）／正在等回合結束 */
  openingPostBusy: boolean;
  openingPostWaiting: boolean;
  onTranslateAndPost: (index: number) => Promise<void>;
}

export function AppDialogs({
  genTableOpen,
  onCloseGenerateTable,
  onGeneratedTable,
  settingsOpen,
  settingsRequestKey,
  versionTab,
  updateDialog,
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
  openingPostBusy,
  openingPostWaiting,
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
          requestKey={settingsRequestKey}
          currentWorld={currentWorld}
          versionTab={versionTab}
        />
      )}

      {updateDialog}

      {/* 換語言後的範例桌詢問疊在設定視窗之上。 */}
      {regenOpen && (
        <Dialog
          title={t("sampleRegenTitle")}
          onDismiss={() => void onAnswerRegen("cancel")}
          backdrop
          start={
            <>
              <button type="button" className="btn" onClick={() => void onAnswerRegen("cancel")}>
                {t("sampleRegenCancel")}
              </button>
              <button type="button" className="btn" onClick={() => void onAnswerRegen("keep")}>
                {t("sampleRegenKeep")}
              </button>
            </>
          }
          end={
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void onAnswerRegen("regen")}
            >
              {t("sampleRegenConfirm")}
            </button>
          }
        >
          <p>{t("sampleRegenBody")}</p>
        </Dialog>
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
        onPostOpening={(text, index) => void onPostOpening(text, index)}
        postBusy={openingPostBusy}
        postWaiting={openingPostWaiting}
        onTranslateAndPost={(index) => void onTranslateAndPost(index)}
        onRetranslate={(index) => void imports.translateOpening(index, true)}
      />
    </>
  );
}
