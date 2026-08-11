use proc_macro::TokenStream;
use proc_macro2::Span;
use std::path::{Path, PathBuf};
use syn::{DeriveInput, LitStr, parse_macro_input};

mod embed;
mod template;

#[proc_macro_derive(Template, attributes(template))]
pub fn derive_template(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match template::expand(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(Embed, attributes(embed))]
pub fn derive_embed(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match embed::expand(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

// ── Attribute parsing ────────────────────────────────────────────────

/// Options shared by `#[template(...)]` and `#[embed(...)]`.
struct Attrs {
    path: String,
    strip_prefix: Option<String>,
    strip_suffix: Option<String>,
    /// Where to find the `textus` crate in the caller's namespace.
    root: syn::Path,
}

fn parse_attrs(input: &DeriveInput, name: &str) -> syn::Result<Attrs> {
    let attr = input
        .attrs
        .iter()
        .find(|a| a.path().is_ident(name))
        .ok_or_else(|| syn::Error::new_spanned(input, format!("missing #[{name}(...)]")))?;

    let (mut path, mut strip_prefix, mut strip_suffix) = (None, None, None);
    let mut root = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("path") {
            path = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("strip_prefix") {
            strip_prefix = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("strip_suffix") {
            strip_suffix = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("root") {
            // Accept both `root = ::path::to::textus` and `root = "::path::to::textus"`
            let value = meta.value()?;
            root = Some(if value.peek(LitStr) {
                value.parse::<LitStr>()?.parse::<syn::Path>()?
            } else {
                value.parse::<syn::Path>()?
            });
        } else {
            return Err(meta.error(
                "unknown option; expected `path`, `strip_prefix`, `strip_suffix` or `root`",
            ));
        }
        Ok(())
    })?;

    Ok(Attrs {
        path: path.ok_or_else(|| syn::Error::new_spanned(attr, "`path` is required"))?,
        strip_prefix,
        strip_suffix,
        root: root.unwrap_or_else(|| syn::parse_quote!(::textus)),
    })
}

// ── Filesystem walk ──────────────────────────────────────────────────

/// Resolves the template directory relative to `CARGO_MANIFEST_DIR`.
fn resolve_dir(attrs: &Attrs, input: &DeriveInput) -> syn::Result<PathBuf> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let dir = Path::new(&manifest).join(&attrs.path);

    if !dir.is_dir() {
        return Err(syn::Error::new_spanned(
            input,
            format!("not a directory: {}", dir.display()),
        ));
    }

    Ok(dir)
}

/// The output path for `file`: its location under `dir`, with the configured
/// prefix and suffix trimmed.
fn rel_path(file: &Path, dir: &Path, attrs: &Attrs) -> String {
    let mut rel = file
        .strip_prefix(dir)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");

    if let Some(prefix) = attrs.strip_prefix.as_deref()
        && let Some(trimmed) = rel.strip_prefix(prefix)
    {
        rel = String::from(trimmed);
    }
    if let Some(suffix) = attrs.strip_suffix.as_deref()
        && let Some(trimmed) = rel.strip_suffix(suffix)
    {
        rel = String::from(trimmed);
    }

    rel
}

fn walk_dir(dir: &Path) -> syn::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)
        .map_err(|e| syn::Error::new(Span::call_site(), format!("{}: {e}", dir.display())))?
    {
        let p = entry
            .map_err(|e| syn::Error::new(Span::call_site(), e.to_string()))?
            .path();
        if p.is_dir() {
            out.extend(walk_dir(&p)?);
        } else {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}
