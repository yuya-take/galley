//! 資料と版の物理削除（コンテナ内で実行する `galley purge` だけが使う）。
//!
//! 版を消したあと、どの版からも参照されなくなったブロブも消す。同じ内容の版（「この版に戻す」で
//! 作った版や、同じ HTML を別の資料に登録した版）が残っていれば、ブロブは残す。

use crate::domain::{blob::BlobHash, revision::Revision, revision::RevisionNumber};

/// 物理削除で DB から消したもの。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PurgedRevisions {
    /// 消した版の番号（新しい順）。
    pub numbers: Vec<RevisionNumber>,
    /// どの版からも参照されなくなり、DB から情報を消したブロブ。ストレージの実体はこのあと消す。
    pub unreferenced_blobs: Vec<BlobHash>,
}

/// 消す版のブロブを、消すものと残すものに分けた結果。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlobDisposal {
    /// 消す版だけが使っているので、版と一緒に消すブロブ。
    pub deleted: Vec<BlobHash>,
    /// 消さない版も使っているので残すブロブと、それを使っている版。
    pub kept: Vec<(BlobHash, Vec<Revision>)>,
}

impl BlobDisposal {
    /// `targets` は消す版、`sharing` は `targets` のブロブのどれかを使っている版（資料をまたいで。
    /// `targets` 自身を含んでよい）。ブロブの順番は `targets` での順番にそろえる。
    pub fn split(targets: &[Revision], sharing: &[Revision]) -> Self {
        let mut disposal = Self::default();
        let mut seen: Vec<&BlobHash> = Vec::new();
        for target in targets {
            let hash = &target.blob_hash;
            if seen.contains(&hash) {
                continue;
            }
            seen.push(hash);
            let users: Vec<Revision> = sharing
                .iter()
                .filter(|revision| {
                    &revision.blob_hash == hash
                        && !targets.iter().any(|target| target.id == revision.id)
                })
                .cloned()
                .collect();
            if users.is_empty() {
                disposal.deleted.push(hash.clone());
            } else {
                disposal.kept.push((hash.clone(), users));
            }
        }
        disposal
    }
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;

    use super::*;
    use crate::domain::{
        revision::{AuthorName, RevisionMessage, RevisionSource},
        shared::{DocumentId, RevisionId},
    };

    fn revision(document_id: DocumentId, number: u32, hash: char) -> Revision {
        Revision {
            id: RevisionId::generate(),
            document_id,
            number: RevisionNumber::new(number).unwrap(),
            blob_hash: BlobHash::parse(&hash.to_string().repeat(64)).unwrap(),
            message: RevisionMessage::parse("更新").unwrap(),
            author_name: AuthorName::parse("佐藤").unwrap(),
            source: RevisionSource::Web,
            restored_from: None,
            created_at: Timestamp::UNIX_EPOCH,
        }
    }

    fn hash(c: char) -> BlobHash {
        BlobHash::parse(&c.to_string().repeat(64)).unwrap()
    }

    #[test]
    fn blobs_only_used_by_targets_are_deleted_once() {
        let document = DocumentId::generate();
        // 第3版は第1版に戻した版（同じブロブ）
        let targets = [
            revision(document, 3, 'a'),
            revision(document, 2, 'b'),
            revision(document, 1, 'a'),
        ];
        let disposal = BlobDisposal::split(&targets, &targets);
        assert_eq!(disposal.deleted, vec![hash('a'), hash('b')]);
        assert!(disposal.kept.is_empty());
    }

    #[test]
    fn blobs_shared_with_remaining_revisions_are_kept() {
        let document = DocumentId::generate();
        let target = revision(document, 2, 'a');
        // 同じ資料の第4版（第2版に戻した版）と、別の資料の第1版が同じ内容
        let restored = revision(document, 4, 'a');
        let other = revision(DocumentId::generate(), 1, 'a');
        let disposal = BlobDisposal::split(
            std::slice::from_ref(&target),
            &[target.clone(), restored.clone(), other.clone()],
        );
        assert!(disposal.deleted.is_empty());
        assert_eq!(disposal.kept, vec![(hash('a'), vec![restored, other])]);
    }
}
