// 資料の登録（HTML ファイルのドラッグ&ドロップ、⌘V の貼り付け、「HTMLを登録」「新しい版を登録」）。
// ダイアログの骨組みは crates/web/src/upload/dialog.rs、サーバーの API は crates/web/src/upload/api.rs。
(() => {
  "use strict";

  const dialog = document.getElementById("upload-dialog");
  const rejected = document.getElementById("upload-rejected");
  const overlay = document.querySelector("[data-upload-drop]");
  if (!dialog || !rejected || !overlay) return;

  const form = dialog.querySelector("form");
  const find = (selector, root = dialog) => root.querySelector(selector);
  const MAX_BYTES = Number(dialog.dataset.maxBytes);
  const AUTHOR_MAX_CHARS = 50;

  // ---- 更新者名（app.js と同じ形で Cookie に保存する。名前は author.rs・app.js と揃える） ----
  const AUTHOR_COOKIE = "galley_author";
  const TEN_YEARS = 60 * 60 * 24 * 365 * 10;
  const saveAuthor = (name) => {
    const secure = location.protocol === "https:" ? "; Secure" : "";
    document.cookie = `${AUTHOR_COOKIE}=${encodeURIComponent(name)}; Path=/; Max-Age=${TEN_YEARS}; SameSite=Lax${secure}`;
  };

  const isTyping = (element) =>
    element instanceof HTMLElement &&
    (element.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(element.tagName));

  // 「24.5 KB」「10MB」（1KB = 1024 バイト。サーバーの文言と揃える）。小数1桁への丸め方は `rounding` で選ぶ。
  // 上限と比べて断るときは、ファイルの大きさを切り上げ（Math.ceil）、上限を切り捨てる（Math.floor）。
  // 上限をわずかに超えただけでも「10.1MB。上限は 10MB」のように、必ず上限より大きく見せるため。
  const formatSize = (bytes, rounding = Math.round) => {
    const tenths = (unit) => String(rounding((bytes * 10) / unit) / 10);
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${tenths(1024)} KB`;
    return `${tenths(1024 * 1024)}MB`;
  };
  const byteLength = (text) => new TextEncoder().encode(text).length;
  const isHtmlFile = (file) => /\.html?$/i.test(file.name) || file.type === "text/html";
  // 貼り付けたテキストがタグを含むときだけ HTML とみなす（ふつうの文章の ⌘V では開かない）
  const looksLikeHtml = (text) => /<(!doctype\s+html|[a-z][a-z0-9-]*)(\s[^<>]*)?\/?>/i.test(text);
  const normalize = (text) => text.normalize("NFKC").toLowerCase();

  // ---- 画面の要素 ----
  const tabs = [...dialog.querySelectorAll("[data-upload-tab]")];
  const panels = [...dialog.querySelectorAll("[data-upload-panel]")];
  const fileInput = find("[data-file-input]");
  const pasteArea = form.elements.html;
  const projectSelect = form.elements.project;
  const titleInput = form.elements.title;
  const messageInput = form.elements.message;
  const authorInput = form.elements.author;
  const submit = find("[data-upload-submit]");
  const submitLabel = find("[data-submit-label]");
  const errorBox = find("[data-upload-error]");
  const otherFilter = find("[data-other-filter]");
  const otherList = find("[data-other-list]");
  const hasProjects = projectSelect.options.length > 0;

  // ---- 状態 ----
  // check は { status: "idle" | "checking" | "ok" | "error" | "rejected", message, title }
  const state = {
    tab: "file",
    file: null, // { name, size, html }
    checks: { file: { status: "idle" }, paste: { status: "idle" } },
    targets: null, // { documents, suggested } または { error }
    target: "new", // "suggested" | "new" | "other"
    otherSlug: "",
    hasPickedTarget: false,
    hasEditedTitle: false,
    isSending: false,
  };
  // 古い応答で上書きしないための番号
  const tokens = { file: 0, paste: 0, targets: 0 };
  let pasteTimer = 0;

  // 登録先の見込み：画面の「HTMLを登録」「新しい版を登録」のボタンが持つ（無い画面ではダイアログで選ぶ）
  const pageContext = () => {
    const element = document.querySelector("[data-upload-project]");
    return {
      project: element?.dataset.uploadProject ?? "",
      document: element?.dataset.uploadDocument ?? "",
    };
  };
  let context = pageContext();
  const hasContextProject = () =>
    context.project !== "" && [...projectSelect.options].some((option) => option.value === context.project);
  const currentProject = () => projectSelect.value;
  const projectName = (slug) =>
    [...projectSelect.options].find((option) => option.value === slug)?.textContent ?? "";

  // ---- サーバーとのやりとり ----
  const statusMessage = (status) => {
    if (status === 413) return `HTML が大きすぎて送れません（上限は ${formatSize(MAX_BYTES, Math.floor)}）`;
    if (status === 403) return "このリクエストは受け付けられません。画面を読み込み直してください";
    return "登録できませんでした。時間をおいて、もう一度試してください";
  };
  const request = async (url, options = {}) => {
    try {
      const response = await fetch(url, { credentials: "same-origin", ...options });
      const isJson = response.headers.get("content-type")?.includes("application/json");
      // 失敗の本文は {"error": {"code", "message", "details"}}（crates/web/src/upload/api.rs）
      const body = isJson ? await response.json().catch(() => null) : null;
      if (response.ok) return { ok: true, body };
      const error = body?.error ?? null;
      return { ok: false, error, message: error?.message ?? statusMessage(response.status) };
    } catch {
      return { ok: false, body: null, message: "サーバーにつながりませんでした。時間をおいて、もう一度試してください" };
    }
  };
  const postJson = (url, payload) =>
    request(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });

  // ---- 内容（ファイル・貼り付け）の検査 ----
  const contentHtml = (tab = state.tab) => (tab === "file" ? state.file?.html ?? "" : pasteArea.value);

  const runCheck = async (tab) => {
    const html = contentHtml(tab);
    const token = ++tokens[tab];
    if (html.trim() === "") {
      state.checks[tab] = { status: "idle" };
      render();
      return;
    }
    state.checks[tab] = { status: "checking" };
    render();
    const result = await postJson("/api/upload/check", { html });
    if (token !== tokens[tab]) return;
    if (result.ok) {
      state.checks[tab] = { status: "ok", title: result.body?.title ?? "" };
      if (!state.hasEditedTitle) titleInput.value = result.body?.title ?? "";
    } else if (result.error?.code === "external_resource") {
      state.checks[tab] = { status: "rejected", message: result.message };
      showRejected(tab, result.error.details);
    } else {
      state.checks[tab] = { status: "error", message: result.message };
    }
    render();
  };

  // 送る前に分かる問題（種類・大きさ・文字コード）を確かめて読む。BOM も含めてファイルのバイト列のまま送る
  const readFile = async (file) => {
    if (!isHtmlFile(file)) return { error: "HTML ファイル（.html）を選んでください" };
    if (file.size > MAX_BYTES) {
      return {
        error: `ファイルが大きすぎます（${formatSize(file.size, Math.ceil)}。上限は ${formatSize(MAX_BYTES, Math.floor)}）`,
      };
    }
    try {
      const bytes = await file.arrayBuffer();
      return { html: new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes) };
    } catch {
      return { error: "UTF-8 で保存した HTML を選んでください（このファイルは文字コードが違います）" };
    }
  };

  // ---- 登録先の候補 ----
  const loadTargets = async () => {
    const project = currentProject();
    const token = ++tokens.targets;
    state.targets = null;
    if (!project) {
      render();
      return;
    }
    render();
    const params = new URLSearchParams({ project });
    if (state.file) params.set("file_name", state.file.name);
    const result = await request(`/api/upload/targets?${params}`);
    if (token !== tokens.targets) return;
    state.targets = result.ok ? result.body : { documents: [], suggested: null, error: result.message };
    if (!state.hasPickedTarget) state.target = suggestedDocument() ? "suggested" : "new";
    render();
  };

  // 「〇〇 の新しい版」に出す資料。資料ビューアではその資料、ほかの画面ではファイル名が一致した資料
  const suggestedDocument = () => {
    const documents = state.targets?.documents ?? [];
    const matched = state.tab === "file" ? state.targets?.suggested ?? null : null;
    const slug = (currentProject() === context.project && context.document) || matched;
    const document = documents.find((item) => item.slug === slug);
    return document ? { ...document, isMatch: document.slug === matched } : null;
  };
  const otherDocuments = () => {
    const suggested = suggestedDocument();
    return (state.targets?.documents ?? []).filter((item) => item.slug !== suggested?.slug);
  };

  // ---- 表示 ----
  const setStatus = (element, check, okText) => {
    element.classList.toggle("upload-status--error", check.status === "error");
    element.textContent =
      check.status === "checking" ? "確かめています…"
      : check.status === "ok" ? okText
      : check.status === "error" || check.status === "rejected" ? check.message
      : "";
  };

  const renderOtherList = () => {
    const terms = normalize(otherFilter.value).split(/\s+/).filter((term) => term !== "");
    const documents = otherDocuments().filter((item) => {
      const title = normalize(item.title);
      return terms.every((term) => title.includes(term));
    });
    otherList.replaceChildren(
      ...documents.map((item) => {
        const li = document.createElement("li");
        const label = document.createElement("label");
        label.className = "upload-other-item";
        const radio = document.createElement("input");
        radio.type = "radio";
        radio.name = "other-document";
        radio.value = item.slug;
        radio.checked = item.slug === state.otherSlug;
        const title = document.createElement("span");
        title.className = "upload-other-title";
        title.textContent = item.title;
        const version = document.createElement("span");
        version.className = "version";
        version.textContent = `第${item.revision_number}版`;
        label.append(radio, title, version);
        li.append(label);
        return li;
      }),
    );
    find("[data-other-empty]").hidden = documents.length > 0;
  };

  const render = () => {
    // タブ
    tabs.forEach((tab) => {
      const isSelected = tab.dataset.uploadTab === state.tab;
      tab.setAttribute("aria-selected", String(isSelected));
      tab.tabIndex = isSelected ? 0 : -1;
    });
    panels.forEach((panel) => { panel.hidden = panel.dataset.uploadPanel !== state.tab; });

    // ファイル
    find("[data-file-empty]").hidden = state.file !== null;
    find("[data-file-chosen]").hidden = state.file === null;
    if (state.file) {
      find("[data-file-name]").textContent = state.file.name;
      find("[data-file-size]").textContent = formatSize(state.file.size);
      setStatus(find("[data-file-status]"), state.checks.file, "そのまま登録できます");
    }
    setStatus(find("[data-paste-status]"), state.checks.paste, "そのまま登録できます");

    // プロジェクト
    const project = currentProject();
    // プロジェクトの画面ではそのプロジェクトに決まっている。ほかの画面では選ぶ
    const isProjectFixed = hasContextProject();
    find("[data-project-field]").hidden = !hasProjects || isProjectFixed;
    find("[data-upload-where]").textContent = isProjectFixed ? `「${projectName(project)}」に登録します` : "";

    // 登録先
    const suggested = suggestedDocument();
    const others = otherDocuments();
    if (state.target === "suggested" && !suggested) state.target = "new";
    if (state.target === "other" && others.length === 0) state.target = "new";
    find("[data-target-suggested]").hidden = !suggested;
    if (suggested) {
      find("[data-suggested-title]").textContent = suggested.title;
      find("[data-suggested-version]").textContent = `第${suggested.revision_number + 1}版`;
      find("[data-suggested-match]").hidden = !suggested.isMatch;
    }
    find("[data-target-other]").hidden = others.length === 0;
    find("[data-other]").hidden = state.target !== "other";
    if (state.target === "other") renderOtherList();
    form.querySelectorAll("input[name=target]").forEach((radio) => { radio.checked = radio.value === state.target; });
    const targetsStatus = find("[data-targets-status]");
    targetsStatus.hidden = !(project && (state.targets === null || state.targets.error));
    targetsStatus.textContent = state.targets?.error ?? "登録先の候補を読み込んでいます…";
    find("[data-title-field]").hidden = state.target !== "new";

    // 登録ボタン
    submitLabel.textContent =
      state.target === "suggested" && suggested ? `第${suggested.revision_number + 1}版として登録`
      : state.target === "other" ? "選んだ資料に登録"
      : "新しい資料として登録";
    const isReady =
      hasProjects &&
      project !== "" &&
      state.targets !== null &&
      state.checks[state.tab].status === "ok" &&
      (state.target !== "other" || state.otherSlug !== "");
    submit.disabled = state.isSending || !isReady;
  };

  const showError = (message) => {
    errorBox.textContent = message;
    errorBox.hidden = message === "";
  };

  // ---- 開く・閉じる ----
  const reset = () => {
    tokens.file += 1;
    tokens.paste += 1;
    clearTimeout(pasteTimer);
    state.file = null;
    state.checks = { file: { status: "idle" }, paste: { status: "idle" } };
    state.target = "new";
    state.otherSlug = "";
    state.hasPickedTarget = false;
    state.hasEditedTitle = false;
    state.isSending = false;
    pasteArea.value = "";
    titleInput.value = "";
    messageInput.value = "";
    otherFilter.value = "";
    fileInput.value = "";
    showError("");
  };

  // ダイアログを開く。開いていなければ、画面の登録先の見込みを読み直して空にしてから開き、true を返す
  const openDialog = ({ tab }) => {
    const isOpening = !dialog.open;
    if (isOpening) {
      context = pageContext();
      reset();
      if (hasContextProject()) projectSelect.value = context.project;
    }
    state.tab = tab;
    render();
    if (isOpening) dialog.showModal();
    return isOpening;
  };

  const focusFirstField = () => {
    if (state.tab === "paste") pasteArea.focus();
    else if (state.file) messageInput.focus();
    else find("[data-action=pick-file]", find("[data-file-empty]")).focus();
  };

  const useFile = async (file) => {
    if (rejected.open) rejected.close();
    const isNewName = state.file?.name !== file.name;
    const isOpening = openDialog({ tab: "file" });
    const token = ++tokens.file;
    state.file = { name: file.name, size: file.size, html: null };
    state.checks.file = { status: "checking" };
    showError("");
    render();
    focusFirstField();
    // 「ファイル名が一致」の候補はファイル名で変わる
    if (isOpening || isNewName) {
      state.hasPickedTarget = false;
      loadTargets();
    }
    const result = await readFile(file);
    if (token !== tokens.file) return;
    if (result.error) {
      state.checks.file = { status: "error", message: result.error };
      render();
      return;
    }
    state.file.html = result.html;
    runCheck("file");
  };

  const usePaste = (text) => {
    const isOpening = openDialog({ tab: "paste" });
    pasteArea.value = text;
    focusFirstField();
    runCheck("paste");
    if (isOpening) loadTargets();
  };

  const openEmpty = () => {
    if (openDialog({ tab: "file" })) loadTargets();
    focusFirstField();
  };

  const closeDialog = () => {
    if (state.isSending) return;
    dialog.close();
  };

  // ---- 登録できない資料 ----
  let rejectedFrom = "file";
  // details は { total, resources: [{ line, kind, code }], ai_request }
  const showRejected = (tab, external) => {
    rejectedFrom = tab;
    const name = tab === "file" ? state.file?.name ?? "" : "貼り付けた HTML";
    const size = tab === "file" ? state.file?.size ?? 0 : byteLength(pasteArea.value);
    find("[data-rejected-name]", rejected).textContent = name;
    find("[data-rejected-size]", rejected).textContent = formatSize(size);
    find("[data-rejected-total]", rejected).textContent = String(external.total);
    // 該当するコードは HTML のソースなので、テキストとして入れる
    find("[data-rejected-rows]", rejected).replaceChildren(
      ...external.resources.map((resource) => {
        const row = document.createElement("tr");
        const line = document.createElement("td");
        line.className = "upload-findings-line";
        line.textContent = String(resource.line);
        const kind = document.createElement("td");
        kind.className = "upload-findings-kind";
        kind.textContent = resource.kind;
        const code = document.createElement("td");
        const codeText = document.createElement("code");
        codeText.textContent = resource.code;
        code.append(codeText);
        row.append(line, kind, code);
        return row;
      }),
    );
    const more = find("[data-rejected-more]", rejected);
    const hidden = external.total - external.resources.length;
    more.hidden = hidden <= 0;
    more.textContent = hidden > 0 ? `ほかに ${hidden} か所あります。` : "";
    find("[data-rejected-request]", rejected).textContent = external.ai_request;
    find("[data-rejected-copy]", rejected).dataset.copyText = external.ai_request;
    find("[data-action=retry-upload]", rejected).textContent =
      tab === "file" ? "別のファイルを選ぶ" : "貼り付け直す";
    state.isSending = false;
    if (dialog.open) dialog.close();
    rejected.showModal();
  };

  rejected.addEventListener("click", (event) => {
    const action = event.target.closest("[data-action]")?.dataset.action;
    if (action === "close-rejected") {
      rejected.close();
    } else if (action === "retry-upload") {
      rejected.close();
      if (rejectedFrom === "file") {
        fileInput.click();
      } else {
        const text = pasteArea.value;
        if (openDialog({ tab: "paste" })) loadTargets();
        pasteArea.value = text;
        render();
        pasteArea.focus();
        pasteArea.select();
      }
    }
  });

  // ---- 登録 ----
  const targetPayload = () => {
    if (state.target === "suggested") return { kind: "revision", document: suggestedDocument()?.slug ?? "" };
    if (state.target === "other") return { kind: "revision", document: state.otherSlug };
    return { kind: "new_document", title: titleInput.value.trim() };
  };

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (state.isSending || submit.disabled) return;
    const author = authorInput.value.trim();
    if (author === "" || [...author].length > AUTHOR_MAX_CHARS) {
      showError(`更新者の名前を入れてください（${AUTHOR_MAX_CHARS}文字まで）`);
      authorInput.focus();
      return;
    }
    const tab = state.tab;
    const payload = {
      project: currentProject(),
      target: targetPayload(),
      file_name: tab === "file" ? state.file?.name ?? null : null,
      html: contentHtml(tab),
      message: messageInput.value.trim(),
    };
    // 送っている間は押せなくする（↵ の連打で版が2つできないように）
    state.isSending = true;
    showError("");
    render();
    saveAuthor(author);
    const result = await postJson("/api/upload", payload);
    if (result.ok && result.body?.url) {
      location.assign(result.body.url);
      return;
    }
    state.isSending = false;
    if (result.error?.code === "external_resource") {
      showRejected(tab, result.error.details);
    } else {
      showError(result.message);
    }
    render();
  });

  // ---- ダイアログの操作 ----
  dialog.addEventListener("click", (event) => {
    const action = event.target.closest("[data-action]")?.dataset.action;
    if (action === "close-upload") closeDialog();
    else if (action === "pick-file") fileInput.click();
  });

  // 送っている間は esc で閉じない
  dialog.addEventListener("cancel", (event) => {
    if (state.isSending) event.preventDefault();
  });

  // ↵ で登録。HTML の欄では改行を入れるので ⌘↵（Ctrl+↵）で登録する。ボタン・リンク・選択欄では ↵ をそのまま使う
  dialog.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" || event.isComposing || event.shiftKey || event.altKey) return;
    const target = event.target;
    const isModified = event.metaKey || event.ctrlKey;
    if (target instanceof HTMLTextAreaElement && !isModified) return;
    if (!isModified && (target instanceof HTMLButtonElement || target instanceof HTMLAnchorElement || target instanceof HTMLSelectElement)) return;
    event.preventDefault();
    form.requestSubmit();
  });

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
      state.tab = tab.dataset.uploadTab;
      if (!state.hasPickedTarget) state.target = suggestedDocument() ? "suggested" : "new";
      render();
      if (state.tab === "paste" && state.checks.paste.status === "idle") runCheck("paste");
    });
    // ← → でタブを切り替える
    tab.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      event.preventDefault();
      const next = tabs[(tabs.indexOf(tab) + (event.key === "ArrowRight" ? 1 : tabs.length - 1)) % tabs.length];
      next.focus();
      next.click();
    });
  });

  fileInput.addEventListener("change", () => {
    const [file] = fileInput.files ?? [];
    if (file) useFile(file);
    fileInput.value = "";
  });

  pasteArea.addEventListener("input", () => {
    clearTimeout(pasteTimer);
    state.checks.paste = pasteArea.value.trim() === "" ? { status: "idle" } : { status: "checking" };
    render();
    pasteTimer = setTimeout(() => runCheck("paste"), 400);
  });

  projectSelect.addEventListener("change", () => {
    state.hasPickedTarget = false;
    state.otherSlug = "";
    loadTargets();
  });

  form.addEventListener("change", (event) => {
    const target = event.target;
    if (target.name === "target") {
      state.target = target.value;
      state.hasPickedTarget = true;
      render();
      if (state.target === "other") otherFilter.focus();
    } else if (target.name === "other-document") {
      state.otherSlug = target.value;
      render();
    }
  });

  titleInput.addEventListener("input", () => { state.hasEditedTitle = true; });
  otherFilter.addEventListener("input", renderOtherList);

  // 「＋ HTMLを登録」「新しい版を登録」
  document.addEventListener("click", (event) => {
    if (!event.target.closest("[data-action=open-upload]")) return;
    openEmpty();
  });

  // ---- ドラッグ&ドロップ ----
  // 資料の iframe（別オリジン）の上ではイベントが親に届かないので、ドラッグが画面に入ったら重ね表示を
  // iframe より上に出して受け止める。既定の動作（ブラウザでファイルを開く）は window で止める
  const hasFiles = (event) => [...(event.dataTransfer?.types ?? [])].includes("Files");
  let dragDepth = 0;

  const dropMessage = (event) => {
    const items = [...(event.dataTransfer?.items ?? [])].filter((item) => item.kind === "file");
    if (!hasProjects) return { title: "まだプロジェクトがありません", note: "先にプロジェクトを作ってから登録してください。" };
    if (items.length > 1) return { title: "1つずつ登録してください", note: "HTML ファイルを1つだけドラッグしてください。" };
    // ドラッグ中はファイル名が分からない。種類が分かるときだけ HTML かを確かめる
    if (items.length === 1 && items[0].type !== "" && items[0].type !== "text/html") {
      return { title: "HTML ファイルだけ登録できます", note: ".html のファイルをドラッグしてください。" };
    }
    const here = pageContext();
    const note = here.document ? "この資料の新しい版として登録します。"
      : here.project ? `「${projectName(here.project)}」に登録します。同じ名前の資料があれば、その新しい版にします。`
      : "次の画面で、登録するプロジェクトを選びます。";
    return { title: "離すと登録できます", note };
  };

  const showOverlay = (event) => {
    const { title, note } = dropMessage(event);
    find("[data-drop-title]", overlay).textContent = title;
    find("[data-drop-note]", overlay).textContent = note;
    overlay.hidden = false;
  };
  const hideOverlay = () => {
    dragDepth = 0;
    overlay.hidden = true;
  };
  // 名前のダイアログや ⌘K の検索の上では受けない（登録のダイアログ・登録できない資料のダイアログでは、ファイルを差し替える）
  const isOtherDialogOpen = () =>
    [...document.querySelectorAll("dialog[open]")].some((open) => open !== dialog && open !== rejected);

  window.addEventListener("dragenter", (event) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    dragDepth += 1;
    if (dragDepth === 1 && !dialog.open && !rejected.open && !isOtherDialogOpen()) showOverlay(event);
  });
  window.addEventListener("dragover", (event) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = hasProjects && !isOtherDialogOpen() ? "copy" : "none";
  });
  window.addEventListener("dragleave", (event) => {
    if (!hasFiles(event)) return;
    dragDepth = Math.max(0, dragDepth - 1);
    if (dragDepth === 0) hideOverlay();
  });
  window.addEventListener("drop", (event) => {
    if (!hasFiles(event)) return;
    event.preventDefault();
    hideOverlay();
    if (!hasProjects || isOtherDialogOpen() || state.isSending) return;
    const files = [...(event.dataTransfer?.files ?? [])];
    if (files.length === 0) return;
    if (files.length > 1) {
      if (openDialog({ tab: "file" })) loadTargets();
      showError("ファイルは1つずつ登録してください");
      return;
    }
    useFile(files[0]);
  });

  // ---- ⌘V の貼り付け ----
  // 入力欄にいるとき・ほかのダイアログが開いているときは、ふつうの貼り付けのままにする
  document.addEventListener("paste", (event) => {
    if (event.defaultPrevented || !hasProjects) return;
    if (isTyping(document.activeElement) || document.querySelector("dialog[open]")) return;
    const data = event.clipboardData;
    const file = [...(data?.files ?? [])].find(isHtmlFile);
    if (file) {
      event.preventDefault();
      useFile(file);
      return;
    }
    const text = data?.getData("text/plain") ?? "";
    if (!looksLikeHtml(text)) return;
    event.preventDefault();
    usePaste(text);
  });
})();
