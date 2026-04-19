#![cfg_attr(not(test), allow(dead_code))]

use std::path::Path;

use maximus_core::{FixFilePreview, PreviewedFix};

pub fn render_fix_preview(target_dir: &Path, previews: &[PreviewedFix]) -> String {
    let mut blocks = Vec::new();

    for preview in previews {
        blocks.push(format!("- {}", preview.title));

        for file_preview in &preview.previews {
            blocks.push(render_file_diff(target_dir, file_preview));
        }
    }

    blocks.join("\n\n")
}

fn render_file_diff(target_dir: &Path, preview: &FixFilePreview) -> String {
    let path = display_preview_path(target_dir, &preview.path);

    if !preview.existed_before {
        return render_create_diff(&path, &preview.after);
    }

    if let Some(addition) = preview.after.strip_prefix(&preview.before) {
        return render_append_diff(&path, &preview.before, addition);
    }

    render_replace_diff(&path, &preview.before, &preview.after)
}

fn render_create_diff(path: &str, after: &str) -> String {
    let after_lines = diff_lines(after);
    let mut lines = vec![
        "--- /dev/null".to_string(),
        format!("+++ {path}"),
        format!("@@ -0,0 +1,{} @@", after_lines.len()),
    ];

    for line in after_lines {
        lines.push(format!("+{line}"));
    }

    lines.join("\n")
}

fn render_append_diff(path: &str, before: &str, addition: &str) -> String {
    let before_lines = diff_lines(before);
    let addition_lines = diff_lines(addition);
    let before_start = if before_lines.is_empty() { 0 } else { 1 };
    let mut lines = vec![
        format!("--- {path}"),
        format!("+++ {path}"),
        format!(
            "@@ -{before_start},{} +1,{} @@",
            before_lines.len(),
            before_lines.len() + addition_lines.len()
        ),
    ];

    for line in before_lines {
        lines.push(format!(" {line}"));
    }

    for line in addition_lines {
        lines.push(format!("+{line}"));
    }

    lines.join("\n")
}

fn render_replace_diff(path: &str, before: &str, after: &str) -> String {
    let before_lines = diff_lines(before);
    let after_lines = diff_lines(after);
    let mut lines = vec![
        format!("--- {path}"),
        format!("+++ {path}"),
        format!("@@ -1,{} +1,{} @@", before_lines.len(), after_lines.len()),
    ];

    for line in before_lines {
        lines.push(format!("-{line}"));
    }

    for line in after_lines {
        lines.push(format!("+{line}"));
    }

    lines.join("\n")
}

fn diff_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut lines = text.split('\n').map(ToString::to_string).collect::<Vec<_>>();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn display_preview_path(root_dir: &Path, path: &Path) -> String {
    path.strip_prefix(root_dir)
        .ok()
        .map(|relative| relative.to_string_lossy().into_owned())
        .filter(|relative| !relative.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use maximus_core::{FixFilePreview, PreviewedFix};

    use super::render_fix_preview;

    #[test]
    fn render_fix_preview_formats_create_and_append_diffs() {
        let target_dir = PathBuf::from("/tmp/project");
        let preview = render_fix_preview(
            &target_dir,
            &[
                PreviewedFix {
                    id: "env-example:create:/tmp/project".to_string(),
                    title: "Create .env.example".to_string(),
                    files: vec![target_dir.join(".env.example")],
                    previews: vec![FixFilePreview {
                        path: target_dir.join(".env.example"),
                        existed_before: false,
                        before: String::new(),
                        after: "API_URL=\n".to_string(),
                    }],
                },
                PreviewedFix {
                    id: "env-example:sync:/tmp/project".to_string(),
                    title: "Append missing keys to .env.example".to_string(),
                    files: vec![target_dir.join(".env.example")],
                    previews: vec![FixFilePreview {
                        path: target_dir.join(".env.example"),
                        existed_before: true,
                        before: "EXISTING=\n".to_string(),
                        after: "EXISTING=\nAPI_URL=\n".to_string(),
                    }],
                },
            ],
        );

        assert!(preview.contains("--- /dev/null"));
        assert!(preview.contains("+++ .env.example"));
        assert!(preview.contains("+API_URL="));
        assert!(preview.contains("@@ -1,1 +1,2 @@"));
        assert!(preview.contains(" EXISTING="));
    }

    #[test]
    fn render_fix_preview_treats_existing_empty_file_as_append() {
        let target_dir = PathBuf::from("/tmp/project");
        let preview = render_fix_preview(
            &target_dir,
            &[PreviewedFix {
                id: "env-example:sync:/tmp/project".to_string(),
                title: "Append missing keys to .env.example".to_string(),
                files: vec![target_dir.join(".env.example")],
                previews: vec![FixFilePreview {
                    path: target_dir.join(".env.example"),
                    existed_before: true,
                    before: String::new(),
                    after: "API_URL=\n".to_string(),
                }],
            }],
        );

        assert!(preview.contains("--- .env.example"));
        assert!(preview.contains("+++ .env.example"));
        assert!(preview.contains("@@ -0,0 +1,1 @@"));
        assert!(!preview.contains("/dev/null"));
    }
}
