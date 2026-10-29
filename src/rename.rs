//! The renaming rules: one validation, one plan, one execution (and undo).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Characters Windows does not allow in a file name.
pub const INVALID_CHARS: &str = "<>:\"/\\|?*";

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn has_invalid(s: &str) -> bool {
    s.chars().any(|c| INVALID_CHARS.contains(c) || c.is_control())
}

/// Trims and collapses runs of white space into single spaces.
pub fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fold(s: &str) -> String {
    s.to_lowercase()
}

/// `name.ext` into (`name`, `.ext`) the way Python's `os.path.splitext` does (a leading dot is
/// part of the name).
pub fn split_ext(name: &str) -> (&str, &str) {
    let lead = name.bytes().take_while(|b| *b == b'.').count();
    let rest = name.get(lead..).unwrap_or("");
    match rest.rfind('.') {
        Some(i) => {
            let at = lead + i;
            (name.get(..at).unwrap_or(name), name.get(at..).unwrap_or(""))
        }
        None => (name, ""),
    }
}

/// The one validation. Returns the file name to use (the original one on error) and the reason
/// the new name is not acceptable, if any.
pub fn validate_base(new_raw: &str, ext: &str, original: &str) -> (String, Option<String>) {
    let cleaned = collapse_spaces(new_raw);
    let fail = |why: String| (original.to_string(), Some(why));
    if cleaned.is_empty() {
        return fail("il nuovo nome sarebbe vuoto (rimarrebbe solo l'estensione)".into());
    }
    if has_invalid(&cleaned) {
        return fail(format!("contiene caratteri non validi {INVALID_CHARS}"));
    }
    let upper = cleaned.to_uppercase();
    let base = upper.split('.').next().unwrap_or("").trim_end_matches(' ');
    if RESERVED.contains(&base) || RESERVED.contains(&upper.trim_end_matches(['.', ' '])) {
        return fail(format!("'{cleaned}' è riservato da Windows"));
    }
    if cleaned.ends_with('.') || cleaned.ends_with(' ') {
        return fail("termina con punto o spazio".into());
    }
    let new_name = format!("{cleaned}{ext}");
    (new_name, None)
}

/// Find and replace inside the name (not the extension), validated.
pub fn safe_new_filename(original: &str, find: &str, replace: &str) -> (String, Option<String>) {
    let same = || (original.to_string(), None);
    if find.is_empty() {
        return same();
    }
    if has_invalid(replace) {
        return (
            original.to_string(),
            Some(format!("il testo di sostituzione contiene caratteri non validi {INVALID_CHARS}")),
        );
    }
    let (name, ext) = split_ext(original);
    if !name.contains(find) {
        return same();
    }
    validate_base(&name.replace(find, replace), ext, original)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Same,
    Changed,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub original: String,
    pub shown: String,
    pub kind: RowKind,
}

/// What each file would be called, with the problems marked (the live preview).
pub fn preview(files: &[String], find: &str, replace: &str) -> Vec<Row> {
    let existing: HashSet<String> = files.iter().map(|f| fold(f)).collect();
    let mut planned: HashSet<String> = HashSet::new();
    let mut rows = Vec::with_capacity(files.len());
    for f in files {
        let (new, err) = safe_new_filename(f, find, replace);
        let (shown, kind) = if let Some(e) = err {
            (format!("⚠ {f} — {e}"), RowKind::Invalid)
        } else if new != *f && fold(&new) != fold(f) && existing.contains(&fold(&new)) {
            (format!("⚠ {new} — esiste già"), RowKind::Invalid)
        } else if new != *f && planned.contains(&fold(&new)) {
            (format!("⚠ {new} — duplicato in questa operazione"), RowKind::Invalid)
        } else if new != *f {
            planned.insert(fold(&new));
            (new, RowKind::Changed)
        } else {
            (new, RowKind::Same)
        };
        rows.push(Row { original: f.clone(), shown, kind });
    }
    rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Op {
    pub original: String,
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Plan {
    pub ops: Vec<Op>,
    pub errors: Vec<String>,
}

/// Adds `f -> new` to the plan unless the name is taken, by a file or by another rename.
fn push_op(
    plan: &mut Plan,
    folder: &Path,
    f: &str,
    new: String,
    existing: &HashSet<String>,
    planned: &mut HashSet<String>,
    report: bool,
) {
    if new == f {
        return;
    }
    if fold(&new) != fold(f) && existing.contains(&fold(&new)) {
        if report {
            plan.errors.push(format!("{f}: esiste già '{new}'"));
        }
        return;
    }
    if planned.contains(&fold(&new)) {
        if report {
            plan.errors.push(format!("{f}: duplicato '{new}'"));
        }
        return;
    }
    planned.insert(fold(&new));
    plan.ops.push(Op { original: f.to_string(), from: folder.join(f), to: folder.join(&new) });
}

fn name_sets(files: &[String]) -> (HashSet<String>, HashSet<String>) {
    (files.iter().map(|f| fold(f)).collect(), HashSet::new())
}

/// Find/replace on every file name (removal is a replace with nothing).
pub fn plan_replace(folder: &Path, files: &[String], find: &str, replace: &str) -> Plan {
    let (existing, mut planned) = name_sets(files);
    let mut plan = Plan::default();
    for f in files {
        let (new, err) = safe_new_filename(f, find, replace);
        if let Some(e) = err {
            if e.contains("sarebbe vuoto") || e.contains("non validi") || e.contains("riservato") {
                plan.errors.push(format!("{f}: {e}"));
            }
            continue;
        }
        push_op(&mut plan, folder, f, new, &existing, &mut planned, true);
    }
    plan
}

/// Files whose whole name would disappear if `find` were removed.
pub fn removal_blockers(files: &[String], find: &str) -> Vec<String> {
    if find.is_empty() {
        return Vec::new();
    }
    files
        .iter()
        .filter(|f| {
            let (name, _) = split_ext(f);
            name.contains(find) && collapse_spaces(&name.replace(find, "")).is_empty()
        })
        .cloned()
        .collect()
}

/// Dots in the names become spaces.
pub fn plan_remove_dots(folder: &Path, files: &[String]) -> Plan {
    let (existing, mut planned) = name_sets(files);
    let mut plan = Plan::default();
    for f in files {
        let (name, ext) = split_ext(f);
        let (new, err) = validate_base(&name.replace('.', " "), ext, f);
        if err.is_none() {
            push_op(&mut plan, folder, f, new, &existing, &mut planned, false);
        }
    }
    plan
}

/// `S01E01`, `s1 e2`, `S01.E01` … become `1x01`.
pub fn convert_sxxe(name: &str) -> String {
    let b: Vec<char> = name.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if matches!(b[i], 's' | 'S') {
            let mut j = i + 1;
            let ds = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > ds {
                let season: String = b[ds..j].iter().collect();
                let mut k = j;
                while k < b.len() && (b[k].is_whitespace() || matches!(b[k], '.' | '_' | '-')) {
                    k += 1;
                }
                if k < b.len() && matches!(b[k], 'e' | 'E') {
                    let mut m = k + 1;
                    let de = m;
                    while m < b.len() && b[m].is_ascii_digit() {
                        m += 1;
                    }
                    if m > de {
                        let episode: String = b[de..m].iter().collect();
                        if let (Ok(s), Ok(e)) = (season.parse::<u64>(), episode.parse::<u64>()) {
                            out.push_str(&format!("{s}x{e:02}"));
                            i = m;
                            continue;
                        }
                    }
                }
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

pub fn plan_sxxe(folder: &Path, files: &[String]) -> Plan {
    let (existing, mut planned) = name_sets(files);
    let mut plan = Plan::default();
    for f in files {
        let (name, ext) = split_ext(f);
        let converted = convert_sxxe(name);
        if converted == name {
            continue;
        }
        let (new, err) = validate_base(&converted, ext, f);
        if err.is_none() {
            push_op(&mut plan, folder, f, new, &existing, &mut planned, false);
        }
    }
    plan
}

/// The files directly inside `folder`, sorted without regard to case.
pub fn list_files(folder: &Path) -> Result<Vec<String>, String> {
    let mut files: Vec<String> = std::fs::read_dir(folder)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    files.sort_by_key(|f| fold(f));
    Ok(files)
}

/// `(new path, old path)` of every file renamed, for undo.
pub type Done = Vec<(PathBuf, PathBuf)>;

fn same_file_name(a: &Path, b: &Path) -> bool {
    match (a.file_name(), b.file_name()) {
        (Some(x), Some(y)) => fold(&x.to_string_lossy()) == fold(&y.to_string_lossy()),
        _ => false,
    }
}

/// Renames the files; never overwrites an existing one.
pub fn execute(ops: &[Op]) -> (Done, Vec<String>) {
    let (mut done, mut errors) = (Vec::new(), Vec::new());
    for op in ops {
        if op.to.exists() && !same_file_name(&op.from, &op.to) {
            errors.push(format!("{}: il file di destinazione esiste già", op.original));
            continue;
        }
        match std::fs::rename(&op.from, &op.to) {
            Ok(()) => done.push((op.to.clone(), op.from.clone())),
            Err(e) => errors.push(format!("{}: {e}", op.original)),
        }
    }
    (done, errors)
}

/// Puts the names back (last rename first). Returns how many were restored and the errors.
pub fn undo(done: &[(PathBuf, PathBuf)]) -> (usize, Vec<String>) {
    let (mut count, mut errors) = (0, Vec::new());
    for (new, old) in done.iter().rev() {
        if !new.exists() {
            continue;
        }
        if old.exists() && !same_file_name(new, old) {
            errors.push(format!(
                "{}: esiste già un file con il nome originale",
                new.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            ));
            continue;
        }
        match std::fs::rename(new, old) {
            Ok(()) => count += 1,
            Err(e) => errors.push(format!(
                "{}: {e}",
                new.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            )),
        }
    }
    (count, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("renamer-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn extensions_split_like_python() {
        assert_eq!(split_ext("a.tar.gz"), ("a.tar", ".gz"));
        assert_eq!(split_ext(".bashrc"), (".bashrc", ""));
        assert_eq!(split_ext("noext"), ("noext", ""));
        assert_eq!(split_ext("a."), ("a", "."));
        assert_eq!(split_ext("..x"), ("..x", ""));
        assert_eq!(split_ext("è.mp3"), ("è", ".mp3"));
    }

    #[test]
    fn validation_rejects_bad_names() {
        assert!(validate_base("  ", ".mkv", "a.mkv").1.unwrap().contains("vuoto"));
        assert!(validate_base("a:b", ".mkv", "a.mkv").1.unwrap().contains("non validi"));
        assert!(validate_base("con", ".txt", "a.txt").1.unwrap().contains("riservato"));
        assert!(validate_base("NUL.backup", ".txt", "a.txt").1.is_some());
        assert!(validate_base("a.", ".txt", "a.txt").1.unwrap().contains("termina"));
        assert_eq!(validate_base("  a   b ", ".mkv", "x.mkv"), ("a b.mkv".to_string(), None));
        // nothing changed: the original name comes back
        assert_eq!(validate_base("a", ".mkv", "a.mkv"), ("a.mkv".to_string(), None));
    }

    #[test]
    fn replace_only_touches_the_name() {
        assert_eq!(safe_new_filename("Film HD.mkv", " HD", ""), ("Film.mkv".to_string(), None));
        // the extension is never searched
        assert_eq!(safe_new_filename("a.mkv", "mkv", "x"), ("a.mkv".to_string(), None));
        assert!(safe_new_filename("a.mkv", "a", "b/c").1.is_some());
        assert_eq!(safe_new_filename("a.mkv", "", "x").1, None);
    }

    #[test]
    fn the_preview_marks_conflicts() {
        let files = s(&["a b.mkv", "a.b.mkv", "ab.mkv", "c.mkv"]);
        // "a b" -> "ab" already exists; "a.b" -> "ab" would be a duplicate of nothing planned
        let rows = preview(&files, " ", "");
        assert_eq!(rows[0].kind, RowKind::Invalid);
        assert!(rows[0].shown.contains("esiste già"));
        assert_eq!(rows[3].kind, RowKind::Same);
        let rows = preview(&s(&["x1.mkv", "x2.mkv"]), "x", "");
        assert_eq!(rows[0].shown, "1.mkv");
        assert_eq!(rows[0].kind, RowKind::Changed);
        let dup = preview(&s(&["a1.mkv", "b1.mkv"]), "a", "b");
        assert_eq!(dup[0].kind, RowKind::Invalid); // b1.mkv exists
        let dup = preview(&s(&["a x.mkv", "a.x.mkv"]), "a", "");
        assert_eq!(dup.iter().filter(|r| r.kind == RowKind::Changed).count(), 2);
    }

    #[test]
    fn s01e01_becomes_1x01() {
        assert_eq!(convert_sxxe("Serie S01E01 HD"), "Serie 1x01 HD");
        assert_eq!(convert_sxxe("s2.e10"), "2x10");
        assert_eq!(convert_sxxe("S1 - E3 x"), "1x03 x");
        assert_eq!(convert_sxxe("Sunset E5"), "Sunset E5");
        assert_eq!(convert_sxxe("S99999999999999999999999E1"), "S99999999999999999999999E1");
        assert_eq!(convert_sxxe("è S01E02 è"), "è 1x02 è");
    }

    #[test]
    fn removing_a_whole_name_is_blocked() {
        let f = s(&["abc.mkv", "abc def.mkv"]);
        assert_eq!(removal_blockers(&f, "abc"), s(&["abc.mkv"]));
        assert!(removal_blockers(&f, "").is_empty());
    }

    #[test]
    fn a_batch_renames_and_undoes() {
        let d = temp("batch");
        for n in ["Film.2020.HD.mkv", "Film 2021.mkv", "Altro.mkv"] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        let files = list_files(&d).unwrap();
        let plan = plan_remove_dots(&d, &files);
        // "Film.2020.HD" -> "Film 2020 HD"; "Altro" and "Film 2021" have no dots in the name
        assert_eq!(plan.ops.len(), 1);
        let (done, errors) = execute(&plan.ops);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(d.join("Film 2020 HD.mkv").exists() && !d.join("Film.2020.HD.mkv").exists());
        let (n, errs) = undo(&done);
        assert_eq!((n, errs.len()), (1, 0));
        assert!(d.join("Film.2020.HD.mkv").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn nothing_is_ever_overwritten() {
        let d = temp("noover");
        std::fs::write(d.join("a.txt"), b"1").unwrap();
        std::fs::write(d.join("b.txt"), b"2").unwrap();
        let ops = vec![Op { original: "a.txt".into(), from: d.join("a.txt"), to: d.join("b.txt") }];
        let (done, errors) = execute(&ops);
        assert!(done.is_empty() && errors.len() == 1);
        assert_eq!(std::fs::read(d.join("b.txt")).unwrap(), b"2");
        // undo refuses to clobber too
        std::fs::write(d.join("c.txt"), b"3").unwrap();
        let (n, errs) = undo(&[(d.join("c.txt"), d.join("a.txt"))]);
        assert_eq!((n, errs.len()), (0, 1));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_plan_reports_empty_names_and_collisions() {
        let d = Path::new("/x");
        let files = s(&["abc.mkv", "abc 2.mkv", "d.mkv"]);
        let p = plan_replace(d, &files, "abc", "");
        assert_eq!(p.errors.len(), 1);
        assert!(p.errors[0].contains("vuoto"));
        assert_eq!(p.ops.len(), 1);
        let p = plan_replace(d, &s(&["a1.mkv", "b1.mkv"]), "a", "b");
        assert!(p.ops.is_empty() && p.errors[0].contains("esiste già"));
    }
}
