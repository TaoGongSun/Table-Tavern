// 「開新桌並匯入」的開桌那一半：建桌、重讀清單、進桌，回傳可以匯入的新桌 id。
// 只有「進了桌而且可寫」才給 id——新桌進不去（未進桌）或進了唯讀／修復桌（已進但不可寫），
// 匯入都不能往下走：前者把清單重讀成最新，後者玩家就停在那張桌上看原因。

export interface EnterResult {
  entered: boolean;
  writable: boolean;
}

export async function openImportTable(
  io: {
    createWorld: (label: string) => Promise<string>;
    refreshWorlds: () => Promise<void>;
    enterTable: (id: string) => Promise<EnterResult>;
  },
  label: string,
): Promise<string | null> {
  const id = await io.createWorld(label);
  await io.refreshWorlds();
  const result = await io.enterTable(id);
  if (!result.entered) {
    await io.refreshWorlds();
    return null;
  }
  return result.writable ? id : null;
}
