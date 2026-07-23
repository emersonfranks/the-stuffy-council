use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

const REVIEW_LOG: &str = ".github/agent-review-log.md";

#[derive(Deserialize)]
struct MarkdownlintConfig {
    globs: Vec<String>,
    ignores: Vec<String>,
}

fn main() {
    let repo_root = parse_repo_root();
    let mut findings = Vec::new();
    let markdown_files = tracked_markdown(&repo_root);

    check_relative_links(&markdown_files, &mut findings);
    check_root_markdown(&markdown_files, &mut findings);
    check_markdownlint_policy(&repo_root, &mut findings);

    if findings.is_empty() {
        println!();
        println!("doc-lint: all checks passed.");
        return;
    }

    eprintln!();
    eprintln!("FAILED ({} issue(s)):", findings.len());
    for finding in findings {
        eprintln!("  {finding}");
    }
    std::process::exit(1);
}

fn parse_repo_root() -> PathBuf {
    let mut arguments = env::args().skip(1);
    let mut repo_root = env::current_dir().expect("current directory");
    while let Some(argument) = arguments.next() {
        if argument != "--repo-root" {
            usage_and_exit();
        }
        let Some(path) = arguments.next() else {
            usage_and_exit();
        };
        repo_root = PathBuf::from(path);
    }
    repo_root.canonicalize().expect("canonical repository root")
}

fn usage_and_exit() -> ! {
    eprintln!("Usage: cargo run --example doc_lint -- --repo-root <path>");
    std::process::exit(2);
}

fn tracked_markdown(repo_root: &Path) -> Vec<(String, PathBuf, String)> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["ls-files", "-z", "--", "*.md"])
        .output()
        .expect("run git ls-files");
    assert!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut files = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            let relative_path = String::from_utf8(path.to_vec()).expect("UTF-8 tracked path");
            let full_path = repo_root.join(&relative_path);
            let contents = fs::read_to_string(&full_path).expect("read tracked Markdown");
            (relative_path.replace('\\', "/"), full_path, contents)
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn check_relative_links(markdown_files: &[(String, PathBuf, String)], findings: &mut Vec<String>) {
    let mut checked = 0;
    for (relative_path, full_path, contents) in markdown_files {
        for link in markdown_links(contents) {
            let target = link
                .split('#')
                .next()
                .unwrap_or("")
                .split('?')
                .next()
                .unwrap_or("");
            if target.is_empty()
                || target.starts_with('#')
                || target.contains("://")
                || target.starts_with("mailto:")
                || target.starts_with("tel:")
                || target.starts_with('<')
            {
                continue;
            }

            checked += 1;
            let decoded = percent_decode(target);
            let resolved = if let Some(repo_relative) = decoded.strip_prefix('/') {
                repo_root_from(full_path, relative_path).join(repo_relative)
            } else {
                full_path.parent().expect("Markdown parent").join(decoded)
            };
            if !resolved.exists() {
                findings.push(format!(
                    "link-integrity: {relative_path} -> {link} (resolved: {})",
                    resolved.display()
                ));
            }
        }
    }
    println!(
        "== relative links: {} tracked files, {checked} checked ==",
        markdown_files.len()
    );
}

fn repo_root_from(full_path: &Path, relative_path: &str) -> PathBuf {
    let mut root = full_path.to_path_buf();
    for _ in Path::new(relative_path).components() {
        root.pop();
    }
    root
}

fn markdown_links(contents: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut in_fence = false;
    let mut fence_marker = "";

    for line in contents.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker = if trimmed.starts_with("```") {
                "```"
            } else {
                "~~~"
            };
            if !in_fence {
                in_fence = true;
                fence_marker = marker;
            } else if marker == fence_marker {
                in_fence = false;
            }
            continue;
        }
        if in_fence {
            continue;
        }

        let bytes = line.as_bytes();
        let mut index = 0;
        let mut in_code = false;
        while index < bytes.len() {
            if bytes[index] == b'`' {
                in_code = !in_code;
                index += 1;
                continue;
            }
            if !in_code
                && index + 1 < bytes.len()
                && bytes[index] == b']'
                && bytes[index + 1] == b'('
            {
                let start = index + 2;
                if let Some(end_offset) = line[start..].find(')') {
                    links.push(line[start..start + end_offset].trim().to_string());
                    index = start + end_offset + 1;
                    continue;
                }
            }
            index += 1;
        }
    }
    links
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push(high * 16 + low);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(output).unwrap_or_else(|_| value.to_string())
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn check_root_markdown(markdown_files: &[(String, PathBuf, String)], findings: &mut Vec<String>) {
    let allowed = HashSet::from(["AGENTS.md", "README.md"]);
    let violations = markdown_files
        .iter()
        .map(|(relative_path, _, _)| relative_path.as_str())
        .filter(|relative_path| !relative_path.contains('/') && !allowed.contains(relative_path))
        .collect::<Vec<_>>();
    for violation in &violations {
        findings.push(format!(
            "root-md-ban: {violation} must live under docs/ or .github/"
        ));
    }
    println!(
        "== root Markdown allowlist: {} violation(s) ==",
        violations.len()
    );
}

fn check_markdownlint_policy(repo_root: &Path, findings: &mut Vec<String>) {
    let config_path = repo_root.join(".markdownlint-cli2.jsonc");
    let review_log_path = repo_root.join(REVIEW_LOG);
    let config_text = fs::read_to_string(config_path).expect("read markdownlint config");
    let config_without_comments = config_text
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let config: MarkdownlintConfig =
        serde_json::from_str(&config_without_comments).expect("parse markdownlint config");

    if config.globs != ["**/*.md"] {
        findings.push("markdownlint-policy: globs must be exactly [\"**/*.md\"]".to_string());
    }
    if config.ignores != [REVIEW_LOG] {
        findings.push(format!(
            "markdownlint-policy: {REVIEW_LOG} must be the only exclusion"
        ));
    }

    let first_line = fs::read_to_string(review_log_path)
        .expect("read review log")
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    if !first_line.starts_with("Last reviewer: ") {
        findings.push(format!(
            "review-log-contract: {REVIEW_LOG} line 1 must start with 'Last reviewer: '"
        ));
    }
    println!("== markdownlint/review-log policy checked ==");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_links_returns_links_outside_code_only() {
        let markdown = r#"
[real](docs/real.md)
`[inline](docs/inline.md)`

```text
[fenced](docs/fenced.md)
```

![image](static/image.png)
"#;

        let links = markdown_links(markdown);

        assert_eq!(links, vec!["docs/real.md", "static/image.png"]);
    }

    #[test]
    fn percent_decode_decodes_paths_and_preserves_invalid_sequences() {
        assert_eq!(percent_decode("docs/My%20File.md"), "docs/My File.md");
        assert_eq!(percent_decode("docs/100%proof.md"), "docs/100%proof.md");
    }
}
