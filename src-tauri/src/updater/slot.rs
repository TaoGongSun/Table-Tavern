//! 檢查、下載、安裝共用的一份更新。正在下載或安裝時，後來的檢查不得覆寫。
//! 已下載還沒安裝不算忙碌：檢查照常進來。同一個版本維持已下載；別的版本改成已檢查。

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    Idle,
    Checked,
    Downloading,
    Downloaded { version: String },
    Installing { version: String },
}

#[derive(Debug)]
pub(crate) struct UpdateSlot<T> {
    update: Option<T>,
    phase: Phase,
}

impl<T> Default for UpdateSlot<T> {
    fn default() -> Self {
        Self {
            update: None,
            phase: Phase::Idle,
        }
    }
}

impl<T: Clone> UpdateSlot<T> {
    /// 只有傳輸進行中才擋檢查。已下載的版本還要能被下一次檢查換掉。
    pub(crate) fn is_busy(&self) -> bool {
        matches!(self.phase, Phase::Downloading | Phase::Installing { .. })
    }

    pub(crate) fn current(&self) -> Option<&T> {
        self.update.as_ref()
    }

    /// 忙碌時不寫。`version` 是這次遠端的版本。
    /// 已下載且遠端仍是同一個版本：維持 Downloaded，不換掉那份更新，才能直接裝。
    /// 已下載但遠端是別的版本：改成 Checked。舊版的版本庫目錄不動，交包 4。
    /// 遠端沒有更新、而手上已有下載：維持 Downloaded，不把驗過的安裝清掉。
    pub(crate) fn store_check(&mut self, update: Option<T>, version: &str) {
        if self.is_busy() {
            return;
        }
        let downloaded = match &self.phase {
            Phase::Downloaded { version } => Some(version.clone()),
            _ => None,
        };
        if let Some(got) = downloaded {
            if let Some(value) = update {
                if got != version {
                    self.update = Some(value);
                    self.phase = Phase::Checked;
                }
            }
            return;
        }
        match update {
            Some(value) => {
                self.update = Some(value);
                self.phase = Phase::Checked;
            }
            None => {
                self.update = None;
                self.phase = Phase::Idle;
            }
        }
    }

    pub(crate) fn begin_download(&mut self) -> Result<T, String> {
        match self.phase {
            Phase::Idle => Err("尚未檢查更新".to_owned()),
            Phase::Downloading => Err("已在下載".to_owned()),
            Phase::Installing { .. } => Err("已在安裝".to_owned()),
            Phase::Checked | Phase::Downloaded { .. } => {
                let update = self
                    .update
                    .clone()
                    .ok_or_else(|| "尚未檢查更新".to_owned())?;
                self.phase = Phase::Downloading;
                Ok(update)
            }
        }
    }

    pub(crate) fn finish_download(&mut self, version: String) {
        if matches!(self.phase, Phase::Downloading) {
            self.phase = Phase::Downloaded { version };
        }
    }

    pub(crate) fn abort_download(&mut self) {
        if matches!(self.phase, Phase::Downloading) {
            self.phase = if self.update.is_some() {
                Phase::Checked
            } else {
                Phase::Idle
            };
        }
    }

    pub(crate) fn begin_install(&mut self, version: &str) -> Result<T, String> {
        match &self.phase {
            Phase::Downloaded { version: got } if got == version => {
                let update = self.update.clone().ok_or_else(|| "尚未下載".to_owned())?;
                self.phase = Phase::Installing {
                    version: version.to_owned(),
                };
                Ok(update)
            }
            Phase::Downloaded { .. } => Err("下載的版本與要安裝的版本不同".to_owned()),
            Phase::Installing { .. } => Err("已在安裝".to_owned()),
            _ => Err("尚未下載".to_owned()),
        }
    }

    /// 這次安裝失敗才退回已下載。版本對不上代表呼叫端沒拿到這次的階段，不動。
    pub(crate) fn abort_install(&mut self, version: &str) {
        if let Phase::Installing { version: got } = &self.phase {
            if got == version {
                self.phase = Phase::Downloaded {
                    version: version.to_owned(),
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn downloaded<'a>(slot: &mut UpdateSlot<&'a str>, version: &'a str) {
        slot.store_check(Some(version), version);
        assert_eq!(slot.begin_download().unwrap(), version);
        slot.finish_download(version.to_owned());
    }

    #[test]
    fn a_busy_check_does_not_replace_the_downloaded_version() {
        let mut slot = UpdateSlot::default();
        assert_eq!(slot.begin_download().unwrap_err(), "尚未檢查更新");
        slot.store_check(Some("0.3.0"), "0.3.0");
        assert_eq!(slot.begin_install("0.3.0").unwrap_err(), "尚未下載");

        assert_eq!(slot.begin_download().unwrap(), "0.3.0");
        assert_eq!(slot.begin_download().unwrap_err(), "已在下載");
        slot.store_check(Some("0.4.0"), "0.4.0");
        slot.store_check(None, "");
        assert_eq!(slot.current(), Some(&"0.3.0"));

        slot.finish_download("0.3.0".to_owned());
        assert!(!slot.is_busy());
        slot.begin_install("0.3.0").unwrap();
        assert_eq!(slot.begin_install("0.3.0").unwrap_err(), "已在安裝");
        slot.store_check(Some("0.9.0"), "0.9.0");
        assert_eq!(slot.current(), Some(&"0.3.0"));
    }

    #[test]
    fn a_downloaded_a_switches_to_b_then_download_and_install_follow_b() {
        let mut slot = UpdateSlot::default();
        downloaded(&mut slot, "0.3.0");
        slot.store_check(Some("0.4.0"), "0.4.0");
        assert_eq!(slot.current(), Some(&"0.4.0"));
        assert_eq!(slot.begin_install("0.3.0").unwrap_err(), "尚未下載");
        assert_eq!(slot.begin_download().unwrap(), "0.4.0");
        slot.finish_download("0.4.0".to_owned());
        assert_eq!(slot.begin_install("0.4.0").unwrap(), "0.4.0");
    }

    #[test]
    fn a_downloaded_a_installs_without_downloading_again_when_the_check_is_still_a() {
        let mut slot = UpdateSlot::default();
        downloaded(&mut slot, "0.3.0");
        slot.store_check(Some("0.3.0"), "0.3.0");
        assert_eq!(slot.current(), Some(&"0.3.0"));
        assert_eq!(slot.begin_install("0.3.0").unwrap(), "0.3.0");
    }

    #[test]
    fn a_failed_download_lets_a_later_check_replace_it() {
        let mut slot = UpdateSlot::default();
        slot.store_check(Some("0.3.0"), "0.3.0");
        slot.begin_download().unwrap();
        slot.abort_download();
        slot.store_check(Some("0.4.0"), "0.4.0");
        assert_eq!(slot.begin_download().unwrap(), "0.4.0");
        slot.abort_download();
        assert_eq!(slot.begin_install("0.4.0").unwrap_err(), "尚未下載");
    }

    #[test]
    fn a_failed_install_keeps_the_version_that_was_downloaded() {
        let mut slot = UpdateSlot::default();
        downloaded(&mut slot, "0.3.0");
        slot.begin_install("0.3.0").unwrap();
        slot.abort_install("0.9.0");
        assert_eq!(slot.begin_install("0.3.0").unwrap_err(), "已在安裝");
        slot.abort_install("0.3.0");
        slot.store_check(None, "");
        assert_eq!(slot.current(), Some(&"0.3.0"));
        assert_eq!(slot.begin_install("0.3.0").unwrap(), "0.3.0");
    }
}
