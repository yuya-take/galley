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

    // 版の履歴のプレビュー欄はサーバーで描き直すとボタンが入れ替わるので、document で受ける
    document.addEventListener("click", (event) => {
      if (!event.target.closest("[data-action=change-author]")) return;
      error.hidden = true;
      dialog.showModal();
      input.select();
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

  // ---- 照合（⌘K の「資料を探す」、サイドバーの「絞り込む」、一覧の「この中を絞り込む」で共通） ----
  // 全角・半角の違いと大文字・小文字をそろえ（NFKC と小文字）、空白で区切った語がすべて含まれれば当たり
  const normalizeText = (text) => text.normalize("NFKC").toLowerCase();
  const searchTerms = (query) => normalizeText(query).split(/\s+/).filter((term) => term !== "");
  const normalizedTexts = new WeakMap();
  // 要素の data-* の文字列を正規化して覚えておく（入力のたびに正規化し直さない）。
  // サーバーで描き直して値が変わったら正規化し直す
  const textOf = (element, key) => {
    const raw = element.dataset[key] ?? "";
    const cached = normalizedTexts.get(element);
    if (cached?.raw === raw) return cached.text;
    const text = normalizeText(raw);
    normalizedTexts.set(element, { raw, text });
    return text;
  };
  const matchesTerms = (text, terms) => terms.every((term) => text.includes(term));

  // ---- サイドバーのプロジェクトの絞り込み ----
  const filter = document.querySelector("[data-project-filter]");
  if (filter) {
    const items = document.querySelectorAll("[data-project-name]");
    filter.addEventListener("input", () => {
      const terms = searchTerms(filter.value);
      items.forEach((item) => {
        item.hidden = !matchesTerms(textOf(item, "projectName"), terms);
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

  // そのほかの操作のメニューは、外を押したら閉じる。メニューの項目（「アーカイブ」など）を押したときも閉じる
  document.addEventListener("click", (event) => {
    document.querySelectorAll("details.menu[open]").forEach((menu) => {
      if (!menu.contains(event.target) || event.target.closest(".menu-item")) menu.open = false;
    });
  });

  // 「この中を絞り込む」：表示中の行を資料名とプロジェクト名で絞る
  const rowFilter = document.querySelector("[data-row-filter]");
  if (rowFilter) {
    const empty = document.querySelector("[data-filter-empty]");
    const count = document.querySelector("[data-row-count]");
    rowFilter.addEventListener("input", () => {
      const terms = searchTerms(rowFilter.value);
      let shown = 0;
      rows().forEach((row) => {
        const matches = matchesTerms(textOf(row, "searchText"), terms);
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

  // ---- 資料を探す（⌘K） ----
  // 候補は、ダイアログを初めて開いたときに shard が読み込む（crates/web/src/search.rs）。ここでは絞り込みと選択だけをする
  const searchDialog = document.getElementById("search-dialog");
  if (searchDialog) {
    const RECENT_COUNT = 10;
    const MAX_SHOWN = 50;
    const searchInput = searchDialog.querySelector("[data-search-input]");
    const searchLabel = searchDialog.querySelector("[data-search-label]");
    const searchStatus = searchDialog.querySelector("[data-search-status]");
    // shard が描き直すと入れ替わるので、そのつど探す
    const results = () => searchDialog.querySelector("[data-search-results]");
    const shownOptions = () => [...searchDialog.querySelectorAll('[role="option"]:not([hidden])')];
    // userAgentData は "macOS"、navigator.platform は "MacIntel" を返す
    const isMac = /mac|iphone|ipad/i.test(navigator.userAgentData?.platform ?? navigator.platform ?? "");

    // Mac 以外はキーの表示を Ctrl K にする
    if (!isMac) {
      document.querySelectorAll("[data-action=search] kbd").forEach((kbd) => { kbd.textContent = "Ctrl K"; });
    }

    // マウスで選んだときはスクロールしない（乗せただけで一覧が動かないように）
    const select = (option, { scroll = true } = {}) => {
      searchDialog.querySelectorAll('[role="option"][aria-selected="true"]').forEach((other) => {
        other.setAttribute("aria-selected", "false");
      });
      if (option) {
        option.setAttribute("aria-selected", "true");
        searchInput.setAttribute("aria-activedescendant", option.id);
        if (scroll) option.scrollIntoView({ block: "nearest" });
      } else {
        searchInput.removeAttribute("aria-activedescendant");
      }
    };

    const applySearch = () => {
      const list = results();
      if (!list?.hasAttribute("data-loaded")) {
        searchLabel.hidden = true;
        searchStatus.textContent = "読み込んでいます…";
        searchStatus.hidden = false;
        select(null);
        return;
      }
      const terms = searchTerms(searchInput.value);
      const isEmptyQuery = terms.length === 0;
      const limit = isEmptyQuery ? RECENT_COUNT : MAX_SHOWN;
      let matched = 0;
      list.querySelectorAll('[role="option"]').forEach((option) => {
        const matches = matchesTerms(textOf(option, "searchText"), terms);
        if (matches) matched += 1;
        option.hidden = !matches || matched > limit;
      });

      const shown = Math.min(matched, limit);
      searchLabel.hidden = shown === 0;
      searchLabel.textContent = isEmptyQuery ? "最近更新した資料" : `${matched}件`;
      let status = "";
      if (matched === 0) {
        status = isEmptyQuery ? "まだ資料がありません。" : `「${searchInput.value.trim()}」に合う資料はありません。`;
      } else if (!isEmptyQuery && matched > limit) {
        status = `ほかに${matched - limit}件あります。語を足して絞り込んでください。`;
      }
      // 候補には上限がある（search.rs の CANDIDATE_LIMIT）。超えたときは、探した範囲を伝える
      const truncatedAt = list.dataset.truncated;
      if (!isEmptyQuery && truncatedAt) {
        status = `${status}${status ? " " : ""}更新が新しい${truncatedAt}件から探しています。さらに古い資料は一覧から探してください。`;
      }
      searchStatus.textContent = status;
      searchStatus.hidden = status === "";
      select(shownOptions()[0] ?? null);
    };

    const openSearch = () => {
      if (!searchDialog.open) {
        // ほかのダイアログ（名前の設定など）を開いているときは重ねない
        if (document.querySelector("dialog[open]")) return;
        searchDialog.showModal();
      }
      searchInput.focus();
      searchInput.select();
      applySearch();
    };

    // 候補が届いたら（shard が描き直したら）、入力中の語で絞り直す。applySearch は候補の属性しか変えないので、
    // childList だけを見れば自分の変更には反応しない
    new MutationObserver(() => applySearch()).observe(
      searchDialog.querySelector("[data-search-region]"),
      { childList: true, subtree: true },
    );

    searchInput.addEventListener("input", applySearch);

    document.querySelectorAll("[data-action=search]").forEach((button) => {
      button.addEventListener("click", openSearch);
    });

    // ⌘K（Mac）/ Ctrl+K（Windows・Linux）。入力欄にいても開き、ブラウザの既定の動き（検索バーへの移動など）は止める
    document.addEventListener("keydown", (event) => {
      // 自動入力などで key の無い keydown が来ることがある
      if (event.isComposing || event.altKey || event.shiftKey || (event.key ?? "").toLowerCase() !== "k") return;
      const isShortcut = isMac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
      if (!isShortcut) return;
      event.preventDefault();
      openSearch();
    });

    // ↑↓ で選び、↵ で開く。esc で閉じるのは dialog の標準の動き
    searchInput.addEventListener("keydown", (event) => {
      // 日本語の変換を確定する ↵ では開かない
      if (event.isComposing || event.keyCode === 229) return;
      const options = shownOptions();
      const index = options.findIndex((option) => option.getAttribute("aria-selected") === "true");
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        if (options.length === 0) return;
        const next = event.key === "ArrowDown"
          ? Math.min(index + 1, options.length - 1)
          : Math.max(index - 1, 0);
        select(options[next]);
      } else if (event.key === "Enter") {
        event.preventDefault();
        const option = options[index];
        if (option) location.href = option.href;
      }
    });

    // マウスを乗せた候補を選ぶ（押すとリンクとして開く）
    searchDialog.addEventListener("mousemove", (event) => {
      const option = event.target.closest('[role="option"]');
      if (option && option.getAttribute("aria-selected") !== "true") select(option, { scroll: false });
    });

    // ダイアログの外（背景）を押したら閉じる
    searchDialog.addEventListener("click", (event) => {
      if (event.target === searchDialog) searchDialog.close();
    });
  }

  // ---- 資料を読む ----
  const viewer = document.querySelector(".viewer");
  if (viewer) {
    // [ と ] で前後の版に移る。資料（iframe）にフォーカスがあるときは、キーが資料に届くので効かない
    document.addEventListener("keydown", (event) => {
      if (event.defaultPrevented || event.isComposing || event.metaKey || event.ctrlKey || event.altKey) return;
      if (document.querySelector("dialog[open]") || isTyping(document.activeElement)) return;
      const href = event.key === "[" ? viewer.dataset.olderHref
        : event.key === "]" ? viewer.dataset.newerHref
        : undefined;
      if (!href) return;
      event.preventDefault();
      location.href = href;
    });
  }

  // ---- 操作した直後の通知 ----
  // 合図のクエリ（data-notice の値。?registered= など）を付けて開くと、サーバーが通知を出す。
  // 再読み込みや URL のコピーで出し直さないよう、表示したら URL から消す
  document.querySelectorAll("[data-notice]").forEach((notice) => {
    const url = new URL(location.href);
    url.searchParams.delete(notice.dataset.notice);
    history.replaceState(history.state, "", url);
    notice.querySelector("[data-action=close-notice]")?.addEventListener("click", () => notice.remove());
  });
})();
