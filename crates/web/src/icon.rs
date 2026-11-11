//! ガレー船のアイコン（「Galley 画面設計」のアイコンの画面）。

/// サイドバーのロゴ（32px 向けの形。帆の縞は2本）。
pub const LOGO_SVG: &str = r##"<svg width="28" height="28" viewBox="0 0 64 64" aria-hidden="true"><defs><clipPath id="logo-sail"><path d="M16 10.5C23 12 35 12 42 10.5C46.5 17 47 25 43 32C35 31 23 31 16 32C18 24.5 18 17 16 10.5Z"/></clipPath></defs><g transform="translate(32 33) scale(0.86) translate(-32 -32)"><path d="M22 40L15.5 54M31 40L24.5 54M40 40L33.5 54" stroke="#EADBB8" stroke-width="4" stroke-linecap="round"/><g clip-path="url(#logo-sail)"><rect x="12" y="8" width="38" height="26" fill="#EADBB8"/><rect x="20" y="8" width="6.5" height="26" fill="#A5301F"/><rect x="33" y="8" width="6.5" height="26" fill="#A5301F"/></g><path d="M3 25C5 31 8 34 13 35H49C52 34.5 55 33 57 31L55 38L62 41.5L50 44H18C10 44 5 37 3 25Z" fill="#D4AF4A"/></g></svg>"##;

/// プロジェクトの紋章（盾の形）。色は `currentColor`（`.crest--red` などで指定する）。
pub const CREST_SVG: &str = r#"<svg class="crest-shape" width="11" height="13" viewBox="0 0 10 12" aria-hidden="true"><path d="M1 1H9V6C9 8.6 7 10.2 5 11C3 10.2 1 8.6 1 6Z" fill="currentColor"/></svg>"#;
