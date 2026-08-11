#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc as sys;

#[cfg(feature = "std")]
extern crate std as sys;

use sys::borrow::Cow;
use sys::vec::Vec;

pub use textus_derive::{Embed, Template};

/// A compiled template that can render its files by substituting struct fields
/// into `{{ var }}` placeholders.
///
/// Derived via `#[derive(Template)]` — see the crate-level docs for usage.
pub trait Template {
    /// Returns each template file as a `(relative_path, rendered_content)` pair.
    fn render(&self) -> Vec<(&'static str, Cow<'static, str>)>;

    /// Writes every rendered template into `target`, creating directories as needed.
    #[cfg(feature = "std")]
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

/// A directory of files embedded verbatim, keyed by relative path.
///
/// Unlike [`Template`], contents are never parsed, so `{{ ... }}` is left
/// exactly as written and files need not be valid UTF-8 — binary assets such
/// as images and fonts work unchanged.
///
/// Derived via `#[derive(Embed)]` — see the crate-level docs for usage.
pub trait Embed {
    /// Returns every embedded file as a `(relative_path, contents)` pair.
    fn iter() -> &'static [(&'static str, &'static [u8])];

    /// Writes every embedded file into `target`, creating directories as needed.
    #[cfg(feature = "std")]
    fn write_into(target: &std::path::Path) -> std::io::Result<()> {
        for &(rel_path, contents) in Self::iter() {
            let abs = target.join(rel_path);

            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(abs, contents)?;
        }

        Ok(())
    }
}

/// Implementation details used by the generated code.
#[doc(hidden)]
pub mod __private {
    pub use crate::sys::borrow::Cow;
    pub use crate::sys::format;
    pub use crate::sys::vec::Vec;
}
