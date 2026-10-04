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
})();
