// 画面のちょっとした動き。Topcoat の $() 式では書けないもの（ブラウザへの保存など）をここに置く。
(() => {
  "use strict";

  // ---- 更新者名 ----
  // ブラウザの Cookie に保存し、サーバーはそれを読んで画面に出す（「自分が更新」の絞り込みにも使う）。
  const AUTHOR_COOKIE = "galley_author";
  const TEN_YEARS = 60 * 60 * 24 * 365 * 10;

  const saveAuthor = (name) => {
    // HTTPS で使うときは Secure を付ける（社内の HTTP でも使えるよう、常には付けない）
    const secure = location.protocol === "https:" ? "; Secure" : "";
    document.cookie = `${AUTHOR_COOKIE}=${encodeURIComponent(name)}; Path=/; Max-Age=${TEN_YEARS}; SameSite=Lax${secure}`;
  };

  const dialog = document.getElementById("author-dialog");
  if (dialog) {
    const form = dialog.querySelector("form");
    const input = dialog.querySelector("input[name=author]");
    const error = dialog.querySelector("[data-error]");

    document.querySelectorAll("[data-action=change-author]").forEach((button) => {
      button.addEventListener("click", () => {
        error.hidden = true;
        dialog.showModal();
        input.select();
      });
    });

    dialog.querySelector("[data-action=cancel]").addEventListener("click", () => dialog.close());

    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const name = input.value.trim();
      if (name === "" || [...name].length > 50) {
        error.hidden = false;
        return;
      }
      saveAuthor(name);
      location.reload();
    });
  }

  // ---- サイドバーのプロジェクトの絞り込み ----
  const filter = document.querySelector("[data-project-filter]");
  if (filter) {
    const items = document.querySelectorAll("[data-project-name]");
    filter.addEventListener("input", () => {
      const query = filter.value.trim().toLowerCase();
      items.forEach((item) => {
        item.hidden = query !== "" && !item.dataset.projectName.toLowerCase().includes(query);
      });
    });
  }

  // ---- コピー（「リンクをコピー」、MCP の登録コマンド） ----
  // navigator.clipboard は HTTPS か localhost でしか使えない。社内の HTTP でも動くよう、選択してコピーする方法も使う
  const copyText = async (text) => {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return;
    }
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.append(area);
    area.select();
    const copied = document.execCommand("copy");
    area.remove();
    if (!copied) throw new Error("コピーできませんでした");
  };

  const copyTimers = new WeakMap();
  // プレビュー欄はサーバーで描き直すとボタンが入れ替わるので、document で受ける
  document.addEventListener("click", (event) => {
    const button = event.target.closest("[data-copy-link], [data-copy-text]");
    if (!button) return;
    const text = button.dataset.copyText ?? new URL(button.dataset.copyLink, location.href).href;
    const label = button.querySelector("[data-copy-label]") ?? button;
    label.dataset.original ??= label.textContent;
    copyText(text).then(
      () => { label.textContent = "コピーしました"; },
      () => { label.textContent = "コピーできませんでした"; },
    );
    clearTimeout(copyTimers.get(button));
    copyTimers.set(button, setTimeout(() => { label.textContent = label.dataset.original; }, 2000));
  });

  // ---- 資料の一覧 ----
  const rows = () => [...document.querySelectorAll(".document-row")];
  const selectedRow = () =>
    document.querySelector('.document-select[aria-current="true"]')?.closest(".document-row") ?? null;
  const openRow = (row) => {
    const href = row?.querySelector("[data-href]")?.dataset.href;
    if (href) location.href = href;
  };
  const isTyping = (element) =>
    element instanceof HTMLElement &&
    (element.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(element.tagName));

  // 行をダブルクリックすると開く（クリックで選ぶのは Topcoat の @click）
  document.addEventListener("dblclick", (event) => {
    const row = event.target.closest(".document-row");
    if (row) openRow(row);
  });

  // ↑↓ で行を選び、↵ で開く。esc でそのほかの操作のメニューを閉じる
  document.addEventListener("keydown", (event) => {
    if (event.defaultPrevented || event.isComposing || event.metaKey || event.ctrlKey || event.altKey) return;
    if (event.key === "Escape") {
      document.querySelectorAll("details.menu[open]").forEach((menu) => {
        menu.open = false;
        menu.querySelector("summary")?.focus();
      });
      return;
    }
    if (document.querySelector("dialog[open]") || isTyping(document.activeElement)) return;
    const active = document.activeElement;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      const visible = rows().filter((row) => !row.hidden);
      if (visible.length === 0) return;
      const index = visible.indexOf(selectedRow());
      const next = event.key === "ArrowDown"
        ? visible[Math.min(index + 1, visible.length - 1)]
        : visible[Math.max(index - 1, 0)];
      event.preventDefault();
      const button = next.querySelector(".document-select");
      button.focus();
      button.click();
    } else if (event.key === "Enter") {
      // 行のボタンにいるとき、または何も操作していないときだけ。ほかのボタンやリンクはそのまま押す
      if (active?.classList.contains("document-select")) {
        event.preventDefault();
        openRow(active.closest(".document-row"));
      } else if (active === document.body || active === null) {
        const row = selectedRow();
        if (row) {
          event.preventDefault();
          openRow(row);
        }
      }
    }
  });

  // そのほかの操作のメニューは、外を押したら閉じる
  document.addEventListener("click", (event) => {
    document.querySelectorAll("details.menu[open]").forEach((menu) => {
      if (!menu.contains(event.target)) menu.open = false;
    });
  });

  // 「この中を絞り込む」：表示中の行を資料名で絞る
  const rowFilter = document.querySelector("[data-row-filter]");
  if (rowFilter) {
    const empty = document.querySelector("[data-filter-empty]");
    const count = document.querySelector("[data-row-count]");
    rowFilter.addEventListener("input", () => {
      const query = rowFilter.value.trim().toLowerCase();
      let shown = 0;
      rows().forEach((row) => {
        const matches = query === "" || row.dataset.title.includes(query);
        row.hidden = !matches;
        if (matches) shown += 1;
      });
      if (empty) empty.hidden = shown > 0;
      if (count) count.textContent = `${shown}件`;
    });
  }

  // プロジェクトの選択は、選んだらすぐ絞り込む
  document.querySelectorAll("[data-autosubmit]").forEach((select) => {
    select.addEventListener("change", () => select.form.requestSubmit());
  });
})();
