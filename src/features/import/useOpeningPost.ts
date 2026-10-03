// 貼出開場白：跨 chat（落到檯面）與 imports（開場白面板）兩邊，App 只接線。
// 真的落到檯面上了才收掉選擇面板（貼失敗時面板留著，玩家可改挑一則或重按）。回合進行中會排在它後面
// （後端持整桌獨占），等待中按鈕就地換成提示、不重複送出；排隊期間換桌或面板換過一輪，開場白照樣
// 落在原桌，但不把事件加進現在這桌的畫面、不關現在的面板。
import type { ChatController } from "../play/useChatController";
import { useTurnWait } from "../play/useTurnWait";
import type { ImportController } from "./useImportController";

export function useOpeningPost({
  worldId,
  chat,
  imports,
}: {
  worldId: string;
  chat: Pick<ChatController, "postOpening" | "isBusy">;
  imports: Pick<
    ImportController,
    "openingsWorldId" | "openingsSource" | "openingsPanelId" | "closeOpenings"
  >;
}) {
  const { run, busy, waiting } = useTurnWait(chat.isBusy, worldId);

  async function post(text: string, index: number) {
    // 面板是舊桌跳出來的（人已換桌）就只收掉，不貼到現在這張桌
    if (imports.openingsWorldId !== worldId) {
      imports.closeOpenings();
      return;
    }
    const panel = imports.openingsPanelId();
    await run(async ({ live, backend }) => {
      const posted = await backend(() =>
        chat.postOpening(text, index, imports.openingsSource, live),
      );
      if (posted && live() && imports.openingsPanelId() === panel) imports.closeOpenings();
    });
  }

  return { post, busy, waiting };
}
