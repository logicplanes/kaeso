#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

const ROOTED: &[&str] = &[
    "crates/",
    "deploy/",
    "e2e/",
    "fuzz/",
    "apps/",
    "packages/",
    ".github/",
    ".cargo/",
    ".sqlx/",
];

const SCANNED: &[&str] = &["crates", "deploy", "e2e", "fuzz", ".github"];

const EXTERNAL: &[&str] = &["http://", "https://", "mailto:"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn skipped_dir(name: &str) -> bool {
    name == "target" || name == "node_modules" || (name.starts_with('.') && name != ".github")
}

fn markdown_files(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap())
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().into_string().unwrap();
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            if !skipped_dir(&name) {
                markdown_files(&path, root, out);
            }
        } else if name.ends_with(".md") {
            out.push(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }
}

fn prose(doc: &str) -> String {
    let mut fence: Option<&str> = None;
    let mut out = String::new();
    for line in doc.lines() {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m));
        match (fence, marker) {
            (None, Some(m)) => fence = Some(m),
            (Some(open), Some(m)) if open == m => fence = None,
            (None, None) => {
                out.push_str(line);
                out.push('\n');
            }
            _ => {}
        }
    }
    out
}

fn backtick_run(text: &[char], at: usize) -> usize {
    text[at..].iter().take_while(|c| **c == '`').count()
}

fn split_code_spans(prose: &str) -> (Vec<String>, String) {
    let text: Vec<char> = prose.chars().collect();
    let mut spans = Vec::new();
    let mut rest = String::new();
    let mut i = 0;
    while i < text.len() {
        if text[i] != '`' {
            rest.push(text[i]);
            i += 1;
            continue;
        }
        let run = backtick_run(&text, i);
        let start = i + run;
        let mut j = start;
        let mut close = None;
        while j < text.len() {
            if text[j] == '`' {
                let other = backtick_run(&text, j);
                if other == run {
                    close = Some(j);
                    break;
                }
                j += other;
            } else {
                j += 1;
            }
        }
        match close {
            Some(end) => {
                let content: String = text[start..end].iter().collect();
                spans.push(content.trim().to_owned());
                rest.push(' ');
                i = end + run;
            }
            None => {
                rest.extend(&text[i..start]);
                i = start;
            }
        }
    }
    (spans, rest)
}

fn repository_paths(spans: &[String]) -> Vec<String> {
    spans
        .iter()
        .map(|s| s.trim_end_matches('/'))
        .filter(|p| ROOTED.iter().any(|r| p.starts_with(r)))
        .filter(|p| !p.contains([' ', '*', '{', '<', '…']))
        .map(str::to_owned)
        .collect()
}

fn link_targets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in text.split("](").skip(1) {
        let mut depth = 0usize;
        let mut end = part.len();
        for (i, c) in part.char_indices() {
            match c {
                '(' => depth += 1,
                ')' if depth == 0 => {
                    end = i;
                    break;
                }
                ')' => depth -= 1,
                _ => {}
            }
        }
        let inner = part[..end].trim();
        let target = match inner.strip_prefix('<') {
            Some(angled) => angled.split('>').next().unwrap_or_default(),
            None => inner.split_whitespace().next().unwrap_or_default(),
        };
        out.push(target.to_owned());
    }
    for line in text.lines() {
        let line = line.trim_start();
        if let Some(close) = line.strip_prefix('[').and_then(|l| l.find("]:")) {
            let target = line[close + 3..].split_whitespace().next();
            out.extend(target.map(str::to_owned));
        }
    }
    for attribute in ["href=\"", "src=\""] {
        for part in text.split(attribute).skip(1) {
            out.extend(part.split('"').next().map(str::to_owned));
        }
    }
    out
}

fn local_target(target: &str) -> Option<&str> {
    if target.is_empty()
        || target.starts_with('#')
        || EXTERNAL.iter().any(|scheme| target.starts_with(scheme))
    {
        return None;
    }
    let path = target.split(['#', '?']).next().unwrap_or_default();
    (!path.is_empty()).then_some(path)
}

fn documents() -> Vec<PathBuf> {
    let root = root();
    let mut docs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
        .map(|p| p.strip_prefix(&root).unwrap().to_path_buf())
        .collect();
    docs.sort();
    for dir in SCANNED {
        if root.join(dir).is_dir() {
            markdown_files(&root.join(dir), &root, &mut docs);
        }
    }
    docs
}

#[test]
fn every_kept_document_is_checked() {
    let docs = documents();
    for expected in [
        "crates/kaeso-kpp/README.md",
        "crates/kaeso-codecs/README.md",
        "crates/kaeso-codecs/fixtures/README.md",
        "crates/kaeso-codecs/fixtures/openai_chat/errors/RECORDED.md",
        "crates/kaeso-core/data/README.md",
        "e2e/README.md",
        "fuzz/README.md",
    ] {
        assert!(
            docs.contains(&PathBuf::from(expected)),
            "{expected} is not among the checked documents: {docs:?}"
        );
    }
    assert!(
        docs.iter().all(|d| d.components().all(
            |c| !["target", "node_modules", ".venv"].contains(&c.as_os_str().to_str().unwrap())
        )),
        "{docs:?}"
    );
}

#[test]
fn documented_code_paths_exist() {
    let root = root();
    let mut missing = Vec::new();
    for doc in documents() {
        let text = std::fs::read_to_string(root.join(&doc)).unwrap();
        let (spans, _) = split_code_spans(&prose(&text));
        for p in repository_paths(&spans) {
            if !root.join(&p).exists() {
                missing.push(format!("{}: {p}", doc.display()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "paths that do not exist:\n{}",
        missing.join("\n")
    );
}

#[test]
fn relative_links_resolve() {
    let root = root();
    let mut broken = Vec::new();
    for doc in documents() {
        let text = std::fs::read_to_string(root.join(&doc)).unwrap();
        let (_, outside_code) = split_code_spans(&prose(&text));
        let dir = root.join(&doc).parent().unwrap().to_path_buf();
        for target in link_targets(&outside_code) {
            if let Some(path) = local_target(&target) {
                let resolved = match path.strip_prefix('/') {
                    Some(rooted) => root.join(rooted),
                    None => dir.join(path),
                };
                if !resolved.exists() {
                    broken.push(format!("{}: {target}", doc.display()));
                }
            }
        }
    }
    assert!(
        broken.is_empty(),
        "links that do not resolve:\n{}",
        broken.join("\n")
    );
}

#[test]
fn the_scanner_finds_paths_and_links_outside_code() {
    let doc = "See `crates/kaeso-core` and ``e2e/README.md``.\n\
               ```\n[skipped](nowhere.md) `crates/nowhere`\n```\n\
               A [guide](docs/gone.md#part \"title\"), <a href=\"LICENSE\">x</a>, \
               [site](https://docs.kaeso.app), [top](#top) and `[code](inline.md)`.\n\
               [ref]: ../NOTICE.md\n";
    let (spans, rest) = split_code_spans(&prose(doc));
    assert_eq!(
        repository_paths(&spans),
        ["crates/kaeso-core", "e2e/README.md"]
    );
    let local: Vec<String> = link_targets(&rest)
        .iter()
        .filter_map(|t| local_target(t).map(str::to_owned))
        .collect();
    assert_eq!(local, ["docs/gone.md", "../NOTICE.md", "LICENSE"]);
}
