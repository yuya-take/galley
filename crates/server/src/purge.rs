//! `galley purge`：資料ごと、または版を1つ、完全に消す。
//!
//! コンテナの中で、動いているサーバーと同じ DB とストレージを開いて消す。サーバーとは別のプロセスなので、
//! 書き込みは SQLite のロック（`BEGIN IMMEDIATE`）で順番を待つ。消す前に消すものを表示して確認を求める。
//! 消した記録は標準出力と tracing（標準エラー）に出す。サーバーのログ（`docker logs`）には入らない。

use std::{
    io::{BufRead, IsTerminal as _, Write},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context as _, bail};
use galley_core::{
    adapter::sqlite::SqliteDatabase,
    app::document::{PurgeDocument, PurgeOutcome, PurgePlan, RevisionLocation},
    domain::{
        blob::BlobHash,
        revision::{Revision, RevisionNumber},
    },
};
use jiff::tz::TimeZone;
use tracing_subscriber::EnvFilter;

use crate::cli::PurgeArgs;

/// 既定のログ。消した記録（このモジュールの info）だけを出し、DB を開くときのログなどは出さない。
const DEFAULT_LOG_FILTER: &str = "warn,galley::purge=info";

/// サーバーと同じ環境変数（`GALLEY_DATA_DIR`、`GALLEY_BLOB_STORE`）で DB とストレージを開いて消す。
pub async fn run(args: PurgeArgs) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        // `docker exec` に -t が無いときは端末ではないので、色の制御文字を出さない
        .with_ansi(std::io::stderr().is_terminal())
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| DEFAULT_LOG_FILTER.into()),
        )
        .init();

    let data_dir = PathBuf::from(
        std::env::var("GALLEY_DATA_DIR").unwrap_or_else(|_| crate::DEFAULT_DATA_DIR.into()),
    );
    let db_path = data_dir.join("galley.db");
    // 開くと空の DB を作ってしまうので、無ければ止める（GALLEY_DATA_DIR の指定違いなど）
    if !db_path.is_file() {
        bail!(
            "DB（{}）がありません。サーバーと同じ GALLEY_DATA_DIR で実行してください",
            db_path.display()
        );
    }
    let database = SqliteDatabase::open(&db_path).await?;
    let blobs = Arc::new(crate::open_blob_store(&data_dir)?);
    let purge = PurgeDocument::new(
        Arc::new(database.projects()),
        Arc::new(database.documents()),
        Arc::new(database.revisions()),
        blobs,
    );
    execute(
        &purge,
        &args,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
    )
    .await
}

/// 消すものを表示し、確認してから消す。`input` は確認の答え、`output` は表示先。
async fn execute(
    purge: &PurgeDocument,
    args: &PurgeArgs,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> anyhow::Result<()> {
    let target = &args.target;
    let plan = purge
        .plan(&target.project, &target.document, target.revision)
        .await?;
    write_plan(output, &plan)?;

    if !args.is_confirmed {
        confirm(input, output)?;
    }

    let outcome = purge.execute(&plan).await?;
    write_outcome(output, &plan, &outcome)?;
    log_outcome(&plan, &outcome);
    if !outcome.failed_blobs.is_empty() {
        bail!(
            "資料の実体 {} 個を消せませんでした。上に出した置き場所から、手で消してください",
            outcome.failed_blobs.len()
        );
    }
    Ok(())
}

/// `yes`（または `y`）の入力を待つ。それ以外の入力や、入力が無い（`docker exec` に `-i` が無い）ときは中止する。
fn confirm(input: &mut impl BufRead, output: &mut impl Write) -> anyhow::Result<()> {
    write!(
        output,
        "\n消したものは元に戻せません。消してよければ yes と入力してください: "
    )?;
    output.flush()?;
    let mut answer = String::new();
    let read = input
        .read_line(&mut answer)
        .context("確認の入力を読めません")?;
    if read == 0 {
        writeln!(output)?;
        bail!(
            "確認の入力が無いので中止しました。docker exec に -it を付けて実行するか、--yes を付けてください"
        );
    }
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "yes" | "y") {
        bail!("中止しました。何も消していません");
    }
    Ok(())
}

fn write_plan(output: &mut impl Write, plan: &PurgePlan) -> anyhow::Result<()> {
    let archived = if plan.document.is_archived() || plan.project.is_archived() {
        "［アーカイブ済み］"
    } else {
        ""
    };
    writeln!(output, "消すもの:")?;
    writeln!(
        output,
        "  資料  {} / {}（{}/{}）{archived}",
        plan.project.name, plan.document.title, plan.project.slug, plan.document.slug
    )?;
    match &plan.revision {
        Some(revision) => writeln!(output, "  版    {}", describe(revision))?,
        None => writeln!(
            output,
            "  版    {} 個（{}。資料ごと消します）",
            plan.revisions.len(),
            number_range(plan.revisions.iter().map(|r| r.number))
        )?,
    }
    writeln!(
        output,
        "  実体  {} 個（どの版からも使われなくなる HTML）",
        plan.blobs.len()
    )?;
    if !plan.kept_blobs.is_empty() {
        writeln!(
            output,
            "残すもの:\n  実体  {} 個（ほかの版も同じ内容なので残します。この内容も消すなら、次の版も消してください）",
            plan.kept_blobs.len()
        )?;
        for kept in &plan.kept_blobs {
            let used_by: Vec<String> = kept.used_by.iter().map(location).collect();
            writeln!(output, "    {}  {}", short(&kept.hash), used_by.join(", "))?;
        }
    }
    Ok(())
}

fn write_outcome(
    output: &mut impl Write,
    plan: &PurgePlan,
    outcome: &PurgeOutcome,
) -> anyhow::Result<()> {
    let numbers = &outcome.purged.numbers;
    writeln!(output)?;
    match &plan.revision {
        Some(_) if numbers.is_empty() => {
            writeln!(output, "版はすでに消されていました。")?;
        }
        Some(_) => writeln!(
            output,
            "{}/{} の {} を消しました。",
            plan.project.slug,
            plan.document.slug,
            number_range(numbers.iter().copied())
        )?,
        None => writeln!(
            output,
            "資料 {}/{} と、その版 {} 個を消しました。",
            plan.project.slug,
            plan.document.slug,
            numbers.len()
        )?,
    }
    writeln!(
        output,
        "資料の実体 {} 個を消しました。",
        outcome.deleted_blobs.len()
    )?;
    if !outcome.reused_blobs.is_empty() {
        writeln!(
            output,
            "資料の実体 {} 個は、消している間に同じ内容の版が登録されたので残しました:",
            outcome.reused_blobs.len()
        )?;
        for hash in &outcome.reused_blobs {
            writeln!(output, "  {}", short(hash))?;
        }
    }
    if !outcome.failed_blobs.is_empty() {
        writeln!(
            output,
            "資料の実体 {} 個を消せませんでした（DB からは消えています）。保存先の中の置き場所:",
            outcome.failed_blobs.len()
        )?;
        for (hash, error) in &outcome.failed_blobs {
            writeln!(output, "  {}  {error}", hash.storage_key())?;
        }
    }
    Ok(())
}

/// 消した記録を tracing に残す（消した資料・版の番号・ブロブのハッシュ）。
fn log_outcome(plan: &PurgePlan, outcome: &PurgeOutcome) {
    let numbers: Vec<u32> = outcome.purged.numbers.iter().map(|n| n.get()).collect();
    let deleted: Vec<&str> = outcome.deleted_blobs.iter().map(BlobHash::as_str).collect();
    let kept: Vec<&str> = outcome.reused_blobs.iter().map(BlobHash::as_str).collect();
    let failed: Vec<&str> = outcome
        .failed_blobs
        .iter()
        .map(|(hash, _)| hash.as_str())
        .collect();
    let scope = if plan.revision.is_some() {
        "版"
    } else {
        "資料"
    };
    tracing::info!(
        project = %plan.project.slug,
        document = %plan.document.slug,
        document_id = %plan.document.id,
        revisions = ?numbers,
        deleted_blobs = ?deleted,
        reused_blobs = ?kept,
        failed_blobs = ?failed,
        "{scope}を物理削除しました"
    );
}

/// 版の説明（「第2版 2026-11-12 10:00 田中「数字を更新」」）。
fn describe(revision: &Revision) -> String {
    let at = revision
        .created_at
        .to_zoned(TimeZone::system())
        .strftime("%Y-%m-%d %H:%M");
    let message = revision.message.as_str();
    let message = if message.is_empty() {
        String::new()
    } else {
        format!("「{message}」")
    };
    format!(
        "第{}版  {at}  {}{message}",
        revision.number, revision.author_name
    )
}

/// 版の番号の範囲（「第1版〜第3版」）。1つなら「第2版」。
fn number_range(numbers: impl Iterator<Item = RevisionNumber>) -> String {
    let numbers: Vec<RevisionNumber> = numbers.collect();
    match (numbers.iter().min(), numbers.iter().max()) {
        (Some(min), Some(max)) if min == max => format!("第{min}版"),
        (Some(min), Some(max)) => format!("第{min}版〜第{max}版"),
        _ => "版なし".to_owned(),
    }
}

/// 版の場所（`sales/plan/v/3`）。`galley purge` にそのまま渡せる形。
fn location(location: &RevisionLocation) -> String {
    format!(
        "{}/{}/v/{}",
        location.project, location.document, location.number
    )
}

/// ハッシュの先頭12文字（表示用）。
fn short(hash: &BlobHash) -> &str {
    &hash.as_str()[..12]
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use galley_core::{
        adapter::blob_store::ObjectStoreBlobStore,
        app::{
            document::{CreateDocument, CreateDocumentInput, SlugSource},
            project::{CreateProject, ProjectInput},
            revision::{AddRevision, RevisionInput},
        },
        domain::{
            blob::BlobStore, document::DocumentRepository, revision::RevisionSource,
            shared::DocumentId, upload::HtmlSizeLimit,
        },
    };

    use super::*;
    use crate::cli::PurgeTarget;

    const V1: &str = "<!doctype html><title>事業計画</title><p>第1版";
    const V2: &str = "<!doctype html><title>事業計画</title><p>第2版";

    struct Fixture {
        database: SqliteDatabase,
        blobs: Arc<ObjectStoreBlobStore>,
        document_id: DocumentId,
    }

    impl Fixture {
        /// `sales/plan`（第1版・第2版）と、第1版と同じ内容の `sales/copy`。
        async fn new() -> anyhow::Result<Self> {
            let database = SqliteDatabase::in_memory().await?;
            let blobs = Arc::new(ObjectStoreBlobStore::in_memory());
            let project = CreateProject::new(Arc::new(database.projects()))
                .execute(&ProjectInput {
                    slug: "sales".to_owned(),
                    name: "営業部".to_owned(),
                    description: String::new(),
                    color: "blue".to_owned(),
                })
                .await?;
            let create = CreateDocument::new(
                Arc::new(database.projects()),
                Arc::new(database.documents()),
                blobs.clone(),
                HtmlSizeLimit::DEFAULT,
            );
            let mut document_id = None;
            for (file_name, html) in [("plan.html", V1), ("copy.html", V1)] {
                let created = create
                    .execute(CreateDocumentInput {
                        project_id: project.id,
                        title: None,
                        slug: SlugSource::FileName(file_name.to_owned()),
                        revision: input(html),
                    })
                    .await?;
                document_id.get_or_insert(created.document.id);
            }
            let document_id = document_id.context("資料がありません")?;
            AddRevision::new(
                Arc::new(database.documents()),
                Arc::new(database.revisions()),
                blobs.clone(),
                HtmlSizeLimit::DEFAULT,
            )
            .execute(document_id, input(V2))
            .await?;
            Ok(Self {
                database,
                blobs,
                document_id,
            })
        }

        /// `answer` を確認の入力にして実行し、表示と結果を返す。
        async fn run(&self, target: &str, answer: Option<&str>) -> (String, anyhow::Result<()>) {
            let args = PurgeArgs {
                target: parse_target(target),
                is_confirmed: answer.is_none(),
            };
            let purge = PurgeDocument::new(
                Arc::new(self.database.projects()),
                Arc::new(self.database.documents()),
                Arc::new(self.database.revisions()),
                self.blobs.clone(),
            );
            let mut input = Cursor::new(answer.unwrap_or_default().as_bytes().to_vec());
            let mut output = Vec::new();
            let result = execute(&purge, &args, &mut input, &mut output).await;
            (String::from_utf8_lossy(&output).into_owned(), result)
        }

        async fn is_document_left(&self) -> anyhow::Result<bool> {
            Ok(self
                .database
                .documents()
                .find_by_id(self.document_id)
                .await?
                .is_some())
        }
    }

    fn parse_target(target: &str) -> PurgeTarget {
        let parts: Vec<&str> = target.split('/').collect();
        PurgeTarget {
            project: parts[0].to_owned(),
            document: parts[1].to_owned(),
            revision: parts.get(3).map(|n| n.parse().unwrap()),
        }
    }

    fn input(html: &str) -> RevisionInput {
        RevisionInput {
            html: html.as_bytes().to_vec(),
            message: "数字を更新".to_owned(),
            author_name: "佐藤".to_owned(),
            source: RevisionSource::Web,
        }
    }

    #[tokio::test]
    async fn shows_plan_and_purges_after_yes() -> anyhow::Result<()> {
        let fx = Fixture::new().await?;
        let (output, result) = fx.run("sales/plan", Some("yes\n")).await;
        result?;
        assert!(
            output.contains("資料  営業部 / 事業計画（sales/plan）"),
            "{output}"
        );
        assert!(
            output.contains("版    2 個（第1版〜第2版。資料ごと消します）"),
            "{output}"
        );
        assert!(output.contains("実体  1 個"), "{output}");
        // 第1版の内容は sales/copy も使っているので残す
        assert!(output.contains("sales/copy/v/1"), "{output}");
        assert!(
            output.contains("消してよければ yes と入力してください"),
            "{output}"
        );
        assert!(
            output.contains("資料 sales/plan と、その版 2 個を消しました。"),
            "{output}"
        );
        assert!(!fx.is_document_left().await?);
        assert!(fx.blobs.get(&BlobHash::of(V1.as_bytes())).await?.is_some());
        assert!(fx.blobs.get(&BlobHash::of(V2.as_bytes())).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn other_answers_or_closed_input_abort() -> anyhow::Result<()> {
        let fx = Fixture::new().await?;
        for answer in ["no\n", "\n", ""] {
            let (_, result) = fx.run("sales/plan", Some(answer)).await;
            assert!(result.is_err(), "{answer:?}");
            assert!(fx.is_document_left().await?);
        }
        let (_, result) = fx.run("sales/plan", Some("")).await;
        let message = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(message.contains("--yes"), "{message}");
        Ok(())
    }

    #[tokio::test]
    async fn yes_option_skips_confirmation() -> anyhow::Result<()> {
        let fx = Fixture::new().await?;
        let (output, result) = fx.run("sales/plan/v/1", None).await;
        result?;
        assert!(output.contains("版    第1版"), "{output}");
        assert!(!output.contains("yes と入力"), "{output}");
        assert!(
            output.contains("sales/plan の 第1版 を消しました。"),
            "{output}"
        );
        assert!(fx.is_document_left().await?);
        Ok(())
    }

    #[tokio::test]
    async fn latest_revision_is_refused_before_asking() -> anyhow::Result<()> {
        let fx = Fixture::new().await?;
        let (output, result) = fx.run("sales/plan/v/2", Some("yes\n")).await;
        let message = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(message.contains("最新の版"), "{message}");
        assert!(output.is_empty(), "{output}");
        Ok(())
    }

    #[test]
    fn number_range_formats() {
        let n = |n| RevisionNumber::new(n).unwrap();
        assert_eq!(number_range([n(2)].into_iter()), "第2版");
        assert_eq!(number_range([n(3), n(1)].into_iter()), "第1版〜第3版");
    }
}
