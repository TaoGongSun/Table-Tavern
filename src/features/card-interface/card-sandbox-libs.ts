// 卡片介面沙盒的內建全域庫：酒館助手的 iframe 本來就有 jQuery（`$`）、lodash（`_`）與 `errorCatched`，
// MVU 前端卡直接用、不自己載。沒有它們 `$(errorCatched(init))` 第一行就拋錯。
// 版本鎖在 package.json（與酒館同代的 jQuery 3／lodash 4），內嵌進 srcdoc，離線也畫得出來。
import jquerySource from "jquery/dist/jquery.min.js?raw";
import lodashSource from "lodash/lodash.min.js?raw";

/** 內嵌進 <script> 的原始碼：關不掉外層 script 元素 */
export function inlineScript(source: string): string {
  return source.replace(/<\/(script)/gi, "<\\/$1").replace(/<!--/g, "<\\!--");
}

/**
 * errorCatched：照酒館助手的語意包裝函式——出錯時通報（沙盒沒有酒館通知，改記 console.error）
 * 後照樣拋出；回傳 Promise 時改回傳「通報後再拋」的接續 Promise。this 與參數照原呼叫傳下去。
 */
export function buildErrorCatchedSource(): string {
  return `
(function () {
  function report(error) {
    console.error("[table-tavern] errorCatched", error);
  }
  window.errorCatched = function (fn) {
    return function () {
      var result;
      try {
        result = fn.apply(this, arguments);
      } catch (error) {
        report(error);
        throw error;
      }
      if (result && typeof result.then === "function") {
        return result.then(undefined, function (error) {
          report(error);
          throw error;
        });
      }
      return result;
    };
  };
})();
`;
}

/** 沙盒最前面那幾支 script：jQuery、lodash、errorCatched */
export function buildSandboxLibs(): string {
  return [jquerySource, lodashSource, buildErrorCatchedSource()]
    .map((source) => `<script>${inlineScript(source)}</script>`)
    .join("");
}
