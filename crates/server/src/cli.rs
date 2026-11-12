//! コマンドラインの引数。引数が無ければサーバーとして起動する（`main`）。
//!
//! サブコマンドは管理用で、コンテナの中で `docker exec galley galley <サブコマンド>` として実行する。
//! 画面と MCP には置かない操作（資料の物理削除）だけをここに置く。

use std::{ffi::OsString, io::Write};

use anyhow::{Context as _, bail};

/// 使い方（`galley help`）。
const USAGE: &str = "\
使い方:
  galley                                        サーバーとして起動する
  galley purge <プロジェクト>/<資料>[/v/<番号>]  資料ごと、または版を1つ、完全に消す
  galley help                                   この説明を出す

サブコマンドはコンテナの中で実行します（例: docker exec -it galley galley purge sales/plan）。
";

/// 使い方（`galley purge --help`）。
const PURGE_USAGE: &str = "\
資料ごと、または版を1つ、完全に消します。消したものは元に戻せません。

使い方:
  galley purge <プロジェクト>/<資料>          資料と、そのすべての版を消す
  galley purge <プロジェクト>/<資料>/v/<番号>  その版だけを消す（最新の版は消せません）

  <プロジェクト>/<資料> は画面の URL のパスと同じです（例: sales/plan、sales/plan/v/3）。
  どの版からも使われなくなった資料の実体（HTML）も消します。ほかの版も使っている実体は残します。
  アーカイブした資料・プロジェクトも消せます。

オプション:
  -y, --yes   確認せずに消す
  -h, --help  この説明を出す

消す前に、消すものを表示して確認を求めます。確認に答えるため、docker exec には -it を付けます:
  docker exec -it galley galley purge sales/plan
";

/// サブコマンド。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    PurgeHelp,
    Purge(PurgeArgs),
}

/// `galley purge` の引数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgeArgs {
    pub target: PurgeTarget,
    /// `--yes`。確認せずに消す。
    pub is_confirmed: bool,
}

/// 消す対象（`<プロジェクト>/<資料>[/v/<番号>]`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgeTarget {
    pub project: String,
    pub document: String,
    /// 版だけを消すときの番号。
    pub revision: Option<u32>,
}

impl Command {
    /// 引数（先頭のプログラム名を除く）を読む。引数が無ければ `None`（サーバーとして起動する）。
    pub fn from_args(args: impl IntoIterator<Item = OsString>) -> anyhow::Result<Option<Command>> {
        let args = args
            .into_iter()
            .map(|arg| {
                arg.into_string()
                    .map_err(|arg| anyhow::anyhow!("引数「{}」を読めません", arg.display()))
            })
            .collect::<anyhow::Result<Vec<String>>>()?;
        let Some((name, rest)) = args.split_first() else {
            return Ok(None);
        };
        let command = match name.as_str() {
            "help" | "-h" | "--help" => Command::Help,
            "purge" => parse_purge(rest)?,
            other => {
                bail!("「{other}」というサブコマンドはありません。galley help で使い方を表示します")
            }
        };
        Ok(Some(command))
    }

    /// サブコマンドを実行する。
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Command::Help => print(USAGE),
            Command::PurgeHelp => print(PURGE_USAGE),
            Command::Purge(args) => crate::purge::run(args).await,
        }
    }
}

fn print(text: &str) -> anyhow::Result<()> {
    std::io::stdout()
        .write_all(text.as_bytes())
        .context("使い方を出せません")
}

fn parse_purge(args: &[String]) -> anyhow::Result<Command> {
    let mut target = None;
    let mut is_confirmed = false;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::PurgeHelp),
            "-y" | "--yes" => is_confirmed = true,
            option if option.starts_with('-') => {
                bail!(
                    "「{option}」というオプションはありません。galley purge --help で使い方を表示します"
                )
            }
            path if target.is_none() => target = Some(PurgeTarget::parse(path)?),
            _ => bail!("消すものは1つだけ指定します。galley purge --help で使い方を表示します"),
        }
    }
    let Some(target) = target else {
        bail!(
            "消す資料を <プロジェクト>/<資料> の形で指定します。galley purge --help で使い方を表示します"
        );
    };
    Ok(Command::Purge(PurgeArgs {
        target,
        is_confirmed,
    }))
}

impl PurgeTarget {
    /// `sales/plan` や `sales/plan/v/3` を読む。画面の URL のパスをそのまま貼れるよう、前後の `/` は無視する。
    fn parse(path: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let (project, document, revision) = match parts.as_slice() {
            [project, document] => (project, document, None),
            [project, document, "v", number] => (project, document, Some(parse_number(number)?)),
            _ => bail!(
                "「{path}」は <プロジェクト>/<資料> か <プロジェクト>/<資料>/v/<番号> の形で指定します"
            ),
        };
        if project.is_empty() || document.is_empty() {
            bail!(
                "「{path}」は <プロジェクト>/<資料> か <プロジェクト>/<資料>/v/<番号> の形で指定します"
            );
        }
        Ok(Self {
            project: (*project).to_owned(),
            document: (*document).to_owned(),
            revision,
        })
    }
}

/// 版の番号。画面の URL と同じく、数字だけで 0 から始まらないもの。
fn parse_number(text: &str) -> anyhow::Result<u32> {
    let is_valid = !text.is_empty()
        && !text.starts_with('0')
        && text.bytes().all(|byte| byte.is_ascii_digit());
    let number = is_valid.then(|| text.parse::<u32>().ok()).flatten();
    number.with_context(|| format!("版の番号「{text}」は 1 以上の数字で指定します"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> anyhow::Result<Option<Command>> {
        Command::from_args(args.iter().map(OsString::from))
    }

    fn purge(project: &str, document: &str, revision: Option<u32>, is_confirmed: bool) -> Command {
        Command::Purge(PurgeArgs {
            target: PurgeTarget {
                project: project.to_owned(),
                document: document.to_owned(),
                revision,
            },
            is_confirmed,
        })
    }

    #[test]
    fn no_args_starts_server() {
        assert_eq!(parse(&[]).unwrap(), None);
    }

    #[test]
    fn help() {
        for args in [&["help"][..], &["--help"], &["-h"]] {
            assert_eq!(parse(args).unwrap(), Some(Command::Help));
        }
        assert_eq!(
            parse(&["purge", "sales/plan", "--help"]).unwrap(),
            Some(Command::PurgeHelp)
        );
    }

    #[test]
    fn purge_document_or_revision() {
        assert_eq!(
            parse(&["purge", "sales/plan"]).unwrap(),
            Some(purge("sales", "plan", None, false))
        );
        assert_eq!(
            parse(&["purge", "/sales/plan/v/3", "--yes"]).unwrap(),
            Some(purge("sales", "plan", Some(3), true))
        );
        assert_eq!(
            parse(&["purge", "-y", "sales/plan/"]).unwrap(),
            Some(purge("sales", "plan", None, true))
        );
    }

    #[test]
    fn rejects_malformed_targets() {
        for target in [
            "sales",
            "sales/",
            "/plan",
            "sales//plan",
            "sales/plan/3",
            "sales/plan/v",
            "sales/plan/v/0",
            "sales/plan/v/03",
            "sales/plan/v/+3",
            "sales/plan/v/99999999999",
            "sales/plan/v/3/x",
        ] {
            assert!(parse(&["purge", target]).is_err(), "{target}");
        }
    }

    #[test]
    fn rejects_unknown_or_extra_args() {
        assert!(parse(&["serve"]).is_err());
        assert!(parse(&["purge"]).is_err());
        assert!(parse(&["purge", "sales/plan", "--force"]).is_err());
        assert!(parse(&["purge", "sales/plan", "sales/memo"]).is_err());
    }
}
