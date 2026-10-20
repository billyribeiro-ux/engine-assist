//! Line diffs for previews (SR-26, SR-35): every line crosses IPC as escaped spans.

use crate::ipc::types::{Diff, DiffHunk, DiffLine, DiffOp, RepoPath};
use crate::render::spans;

/// Above this many lines on either side, the diff is one replace hunk (no LCS table).
const LCS_LIMIT: usize = 2000;
/// Unchanged lines kept around each change.
const CONTEXT: usize = 3;

fn line(op: DiffOp, text: &str) -> DiffLine {
    DiffLine {
        op,
        spans: spans(text),
    }
}

fn ops<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<(DiffOp, &'a str, usize, usize)> {
    let (n, m) = (old.len(), new.len());
    if n > LCS_LIMIT || m > LCS_LIMIT {
        let mut out: Vec<_> = old
            .iter()
            .enumerate()
            .map(|(i, l)| (DiffOp::Del, *l, i, 0))
            .collect();
        out.extend(new.iter().enumerate().map(|(j, l)| (DiffOp::Add, *l, n, j)));
        return out;
    }
    // lcs[i][j] = LCS length of old[i..] and new[j..].
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if old[i] == new[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::with_capacity(n + m);
    while i < n || j < m {
        if i < n && j < m && old[i] == new[j] {
            out.push((DiffOp::Ctx, old[i], i, j));
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            out.push((DiffOp::Add, new[j], i, j));
            j += 1;
        } else {
            out.push((DiffOp::Del, old[i], i, j));
            i += 1;
        }
    }
    out
}

fn u32_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The diff from `old` (`None`: the file doesn't exist) to `new`, in hunks with context.
#[must_use]
pub fn diff(path: &str, old: Option<&str>, new: &str) -> Diff {
    let old_lines: Vec<&str> = old.map(|o| o.lines().collect()).unwrap_or_default();
    let new_lines: Vec<&str> = new.lines().collect();
    let all = ops(&old_lines, &new_lines);
    let changed: Vec<usize> = all
        .iter()
        .enumerate()
        .filter(|(_, (op, ..))| *op != DiffOp::Ctx)
        .map(|(k, _)| k)
        .collect();
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut k = 0;
    while k < changed.len() {
        let start = changed[k].saturating_sub(CONTEXT);
        let mut end = changed[k];
        while k < changed.len() && changed[k] <= end + 2 * CONTEXT + 1 {
            end = changed[k];
            k += 1;
        }
        let end = (end + CONTEXT).min(all.len() - 1);
        let (_, _, oi, nj) = all[start];
        hunks.push(DiffHunk {
            old_start: u32_of(oi + 1),
            new_start: u32_of(nj + 1),
            lines: all[start..=end]
                .iter()
                .map(|(op, text, ..)| line(*op, text))
                .collect(),
        });
    }
    Diff {
        path: RepoPath::from_trusted(path),
        exists: old.is_some(),
        hunks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_file_is_all_adds() {
        let d = diff("a.md", None, "x\ny\n");
        assert!(!d.exists);
        assert_eq!(d.hunks.len(), 1);
        assert!(d.hunks[0].lines.iter().all(|l| l.op == DiffOp::Add));
    }

    #[test]
    fn change_in_middle_has_context() {
        let old = (1..=20).map(|n| format!("{n}\n")).collect::<String>();
        let new = old.replace("10\n", "ten\n");
        let d = diff("a.md", Some(&old), &new);
        assert_eq!(d.hunks.len(), 1);
        let ops: Vec<DiffOp> = d.hunks[0].lines.iter().map(|l| l.op).collect();
        assert_eq!(ops.iter().filter(|o| **o == DiffOp::Ctx).count(), 6);
        assert_eq!(d.hunks[0].old_start, 7);
        assert!(diff("a.md", Some("same\n"), "same\n").hunks.is_empty());
    }

    #[test]
    fn bidi_is_escaped() {
        let d = diff("a.md", None, "a\u{202e}b\n");
        assert!(matches!(
            d.hunks[0].lines[0].spans[1],
            crate::ipc::types::TextSpan::Escape { .. }
        ));
    }
}
