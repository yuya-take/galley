//! 書体。Fontsource の書体を `topcoat asset bundle` のときにダウンロードし、自前で配信する
//! （画面を開くときに外部の CDN に取りに行かない）。
//!
//! 日本語は文字の範囲で分割されていない1ファイル（1〜2MB）なので、使う太さだけにする。

use topcoat::font::{Font, fontsource::fontsource_font};

/// ロゴ。
pub const LOGO: Font = fontsource_font!(
    UNIFRAKTURMAGUNTIA,
    weight: [400],
    subset: [Latin],
    host: Asset,
);

/// 見出し、資料名。
pub const HEADING: Font = fontsource_font!(
    ZEN_OLD_MINCHO,
    weight: [700, 900],
    subset: [Latin, Japanese],
    host: Asset,
);

/// 本文、操作部品。
pub const BODY: Font = fontsource_font!(
    NOTO_SANS_JP,
    weight: [400, 700],
    subset: [Latin, Japanese],
    host: Asset,
);

/// キー表示（⌘K など）、コマンド。
pub const MONO: Font = fontsource_font!(
    GEIST_MONO,
    weight: [400, 500],
    style: Normal,
    subset: [Latin],
    host: Asset,
);

pub const ALL: [Font; 4] = [LOGO, HEADING, BODY, MONO];
