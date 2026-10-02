// 測試通道的頁面端 DOM 輔助：每次 eval 都以參數 H 傳給使用者程式碼。只在 test-harness 測試包注入。
// target 寫法：CSS selector、`text=完整可見文字`、`text*=部分文字`、`role=button[name="名稱"]`、`role=button[name*="部分"]`。
(() => {
  if (window.__ttHarness) return window.__ttHarness;

  const norm = (s) => String(s ?? "").replace(/\s+/g, " ").trim();

  const IMPLICIT_ROLE = {
    BUTTON: "button",
    SELECT: "combobox",
    TEXTAREA: "textbox",
    OPTION: "option",
    DIALOG: "dialog",
  };

  function roleOf(el) {
    const explicit = el.getAttribute("role");
    if (explicit) return explicit.split(/\s+/)[0];
    if (el.tagName === "A" && el.hasAttribute("href")) return "link";
    if (el.tagName === "INPUT") {
      const type = (el.getAttribute("type") || "text").toLowerCase();
      if (["button", "submit", "reset"].includes(type)) return "button";
      if (type === "checkbox") return "checkbox";
      if (type === "radio") return "radio";
      if (type === "file") return "file";
      return "textbox";
    }
    return IMPLICIT_ROLE[el.tagName] || null;
  }

  function nameOf(el) {
    const labelled = el.getAttribute("aria-labelledby");
    if (labelled) {
      const text = labelled
        .split(/\s+/)
        .map((id) => document.getElementById(id)?.innerText ?? "")
        .join(" ");
      if (norm(text)) return norm(text);
    }
    return norm(
      el.getAttribute("aria-label") ||
        el.innerText ||
        (el.labels && el.labels[0] && el.labels[0].innerText) ||
        el.getAttribute("placeholder") ||
        el.getAttribute("title") ||
        el.value ||
        "",
    );
  }

  /** checkVisibility 不存在時（較舊 WebKit）退回逐層查 computed style。 */
  function visible(el) {
    if (!el.isConnected) return false;
    if (typeof el.checkVisibility === "function") {
      if (!el.checkVisibility({ checkVisibilityCSS: true, checkOpacity: false })) return false;
    } else {
      for (let node = el; node && node.nodeType === 1; node = node.parentElement) {
        const style = getComputedStyle(node);
        if (style.display === "none") return false;
        if (node === el && (style.visibility === "hidden" || style.visibility === "collapse")) return false;
      }
    }
    const rect = el.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }

  /** text= 常選到按鈕裡的 span：往上找實際控制項（自己或任一祖先原生 :disabled、
   *  disabled fieldset／optgroup 繼承、aria-disabled）都算 disabled。 */
  function disabled(el) {
    return el.closest(":disabled, [aria-disabled='true']") !== null;
  }

  /** 最內層：自己符合、且沒有任何後代也符合的元素（避免整串祖先一起被 text= 選到）。 */
  function innermost(list) {
    return list.filter((el) => !list.some((other) => other !== el && el.contains(other)));
  }

  function parseRole(spec) {
    const m = /^([\w-]+)(?:\[name(\*?)=(?:"([^"]*)"|'([^']*)'|([^\]]*))\])?$/.exec(spec.trim());
    if (!m) throw new Error("role= 寫法：role=button、role=button[name=\"名稱\"]、role=button[name*=\"部分\"]：" + spec);
    return { role: m[1], contains: m[2] === "*", name: m[3] ?? m[4] ?? m[5] };
  }

  function find(target) {
    const all = () => [...document.querySelectorAll("body *")];
    if (target.startsWith("text=")) {
      const want = norm(target.slice(5));
      return innermost(all().filter((el) => visible(el) && norm(el.innerText) === want));
    }
    if (target.startsWith("text*=")) {
      const want = norm(target.slice(6));
      return innermost(all().filter((el) => visible(el) && norm(el.innerText).includes(want)));
    }
    if (target.startsWith("role=")) {
      const { role, name, contains } = parseRole(target.slice(5));
      const want = name === undefined ? undefined : norm(name);
      return all().filter(
        (el) =>
          roleOf(el) === role &&
          (want === undefined || (contains ? nameOf(el).includes(want) : nameOf(el) === want)),
      );
    }
    return [...document.querySelectorAll(target)];
  }

  function describe(el) {
    if (!el) return null;
    return {
      tag: el.tagName.toLowerCase(),
      role: roleOf(el),
      name: nameOf(el).slice(0, 80),
      id: el.id || undefined,
      disabled: disabled(el),
      visible: visible(el),
    };
  }

  /** 中心點最上層的元素是目標或其後代才算點得到；先捲進畫面。 */
  function hitTest(el) {
    el.scrollIntoView({ block: "center", inline: "center" });
    const rect = el.getBoundingClientRect();
    const top = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
    return { ok: top !== null && (top === el || el.contains(top)), top };
  }

  function one(target, { hit = true, mustBeVisible = true } = {}) {
    const matches = find(target);
    if (matches.length !== 1) {
      const sample = matches.slice(0, 5).map(describe);
      throw new Error(
        "target 匹配 " + matches.length + " 個（需剛好 1 個）：" + target +
          (sample.length ? " → " + JSON.stringify(sample) : ""),
      );
    }
    const el = matches[0];
    if (disabled(el)) throw new Error("target 是 disabled：" + target);
    if (mustBeVisible && !visible(el)) throw new Error("target 不可見：" + target);
    if (hit) {
      const { ok, top } = hitTest(el);
      if (!ok) throw new Error("target 被遮擋：" + target + " → 最上層是 " + JSON.stringify(describe(top)));
    }
    return el;
  }

  function setNativeValue(el, value) {
    const proto = Object.getPrototypeOf(el);
    const setter = Object.getOwnPropertyDescriptor(proto, "value")?.set;
    if (!setter) throw new Error("這個元素沒有 value：" + el.tagName);
    setter.call(el, value);
  }

  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  const H = {
    find,
    describe,
    query(target) {
      return find(target).map(describe);
    },
    text(target) {
      if (!target) return document.body.innerText;
      return find(target).filter(visible).map((el) => el.innerText);
    },
    click(target) {
      const el = one(target);
      el.click();
      return describe(el);
    },
    fill(target, value) {
      const el = one(target);
      if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement))
        throw new Error("fill 只接受 input／textarea：" + target);
      el.focus();
      setNativeValue(el, value);
      el.dispatchEvent(new Event("input", { bubbles: true }));
      el.dispatchEvent(new Event("change", { bubbles: true }));
      return { ...describe(el), value: el.value };
    },
    select(target, value) {
      const el = one(target);
      if (!(el instanceof HTMLSelectElement)) throw new Error("select 只接受 <select>：" + target);
      const options = [...el.options];
      const option =
        options.find((o) => o.value === value) || options.find((o) => norm(o.text) === norm(value));
      if (!option)
        throw new Error(
          "選項不存在：" + value + " → 可選 " + JSON.stringify(options.map((o) => [o.value, norm(o.text)])),
        );
      // option 在 disabled optgroup 裡時本身 :disabled 也成立
      if (option.matches(":disabled")) throw new Error("選項是 disabled：" + value);
      setNativeValue(el, option.value);
      el.dispatchEvent(new Event("input", { bubbles: true }));
      el.dispatchEvent(new Event("change", { bubbles: true }));
      return { ...describe(el), value: el.value, text: norm(option.text) };
    },
    /** 送出 target 所屬的表單（requestSubmit：跑原生驗證、觸發 submit 事件）。
     *  合成的 Enter 是 untrusted，瀏覽器不會替它送出表單，要送出就用這個。 */
    submit(target) {
      const el = one(target);
      const form = el instanceof HTMLFormElement ? el : el.form || el.closest("form");
      if (!form) throw new Error("target 不在任何 form 裡：" + target);
      form.requestSubmit();
      return describe(el);
    },
    /** 只派給 app 自己的鍵盤處理器；瀏覽器預設行為（Tab 移焦等）不會發生。 */
    press(target, key) {
      const el = one(target);
      el.focus();
      for (const type of ["keydown", "keyup"]) {
        el.dispatchEvent(new KeyboardEvent(type, { key, bubbles: true, cancelable: true }));
      }
      return { target: describe(el), active: describe(document.activeElement) };
    },
    async wait(target, { gone = false, timeout = 10000 } = {}) {
      const deadline = Date.now() + timeout;
      for (;;) {
        const count = find(target).filter(visible).length;
        if (gone ? count === 0 : count > 0) return { target, count };
        if (Date.now() > deadline)
          throw new Error((gone ? "等待消失逾時：" : "等待出現逾時：") + target + "（目前 " + count + " 個）");
        await sleep(100);
      }
    },
    file(target, name, type, base64) {
      const el = one(target, { hit: false, mustBeVisible: false });
      if (!(el instanceof HTMLInputElement) || el.type !== "file")
        throw new Error("target 不是 input[type=file]：" + target);
      const binary = atob(base64);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
      const file = new File([bytes], name, { type });
      const transfer = new DataTransfer();
      transfer.items.add(file);
      el.files = transfer.files;
      el.dispatchEvent(new Event("change", { bubbles: true }));
      return { name: file.name, type: file.type, size: file.size };
    },
    invoke(command, args) {
      return window.__TAURI_INTERNALS__.invoke(command, args ?? {});
    },
  };
  window.__ttHarness = H;
  return H;
})()
