use std::borrow::Cow;

pub use textus_derive::Template;

/// A compiled template that can render its files by substituting struct fields
/// into `{{ var }}` placeholders.
///
/// Derived via `#[derive(Template)]` — see the crate-level docs for usage.
pub trait Template {
    /// Returns each template file as a `(relative_path, rendered_content)` pair.
    fn render(&self) -> Vec<(&'static str, Cow<'static, str>)>;

    /// Writes every rendered template into `target`, creating directories as needed.
    fn render_into(&self, target: &std::path::Path) -> std::io::Result<()> {
        for (rel_path, content) in self.render() {
            let abs = target.join(rel_path);

            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(abs, content.as_ref())?;
        }

        Ok(())
    }
}
