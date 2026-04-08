use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use syn::{DeriveInput, Ident, LitStr, parse_macro_input};

#[proc_macro_derive(Template, attributes(template))]
pub fn derive_template(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

// ── Attribute types ──────────────────────────────────────────────────

enum Mode {
    Strict,
    Default,
    Lenient,
}

struct Attrs {
    path: String,
    mode: Mode,
    strip_prefix: Option<String>,
    strip_suffix: Option<String>,
}

enum Seg {
    Lit(String),
    Var(String),
}

struct FileEntry {
    rel: String,
    segs: Vec<Seg>,
    abs: String,
}

// ── Attribute parsing ────────────────────────────────────────────────

fn parse_attrs(input: &DeriveInput) -> syn::Result<Attrs> {
    let attr = input
        .attrs
        .iter()
        .find(|a| a.path().is_ident("template"))
        .ok_or_else(|| syn::Error::new_spanned(input, "missing #[template(...)]"))?;

    let (mut path, mut mode, mut strip_prefix, mut strip_suffix) = (None, Mode::Default, None, None);
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("path") {
            path = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("strip_prefix") {
            strip_prefix = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("strip_suffix") {
            strip_suffix = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("mode") {
            let lit = meta.value()?.parse::<LitStr>()?.value();
            mode = match lit.as_str() {
                "strict" => Mode::Strict,
                "default" => Mode::Default,
                "lenient" => Mode::Lenient,
                _ => return Err(meta.error(format!("unknown mode `{lit}`"))),
            };
        }
        Ok(())
    })?;

    Ok(Attrs {
        path: path.ok_or_else(|| syn::Error::new_spanned(attr, "`path` is required"))?,
        mode,
        strip_prefix,
        strip_suffix,
    })
}

// ── Template parsing ─────────────────────────────────────────────────

fn parse_template(src: &str) -> Vec<Seg> {
    let mut segs = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find("{{") {
        if i > 0 {
            segs.push(Seg::Lit(rest[..i].into()));
        }
        rest = &rest[i + 2..];
        match rest.find("}}") {
            Some(j) => {
                segs.push(Seg::Var(rest[..j].trim().into()));
                rest = &rest[j + 2..];
            }
            None => segs.push(Seg::Lit("{{".into())),
        }
    }
    if !rest.is_empty() {
        segs.push(Seg::Lit(rest.into()));
    }
    segs
}

fn collect_vars(segs: &[Seg]) -> HashSet<String> {
    segs.iter()
        .filter_map(|s| match s {
            Seg::Var(v) => Some(v.clone()),
            _ => None,
        })
        .collect()
}

// ── Filesystem walk ──────────────────────────────────────────────────

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

// ── Code generation ──────────────────────────────────────────────────

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let attrs = parse_attrs(&input)?;
    let name = &input.ident;
    let (impl_g, ty_g, where_cl) = input.generics.split_for_impl();

    // Collect struct field names
    let fields: HashSet<String> = match &input.data {
        syn::Data::Struct(s) => match &s.fields {
            syn::Fields::Named(n) => n
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap().to_string())
                .collect(),
            _ => return Err(syn::Error::new_spanned(&input, "named fields required")),
        },
        _ => return Err(syn::Error::new_spanned(&input, "only structs supported")),
    };

    // Resolve template directory relative to CARGO_MANIFEST_DIR
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let dir = Path::new(&manifest).join(&attrs.path);
    if !dir.is_dir() {
        return Err(syn::Error::new_spanned(
            &input,
            format!("not a directory: {}", dir.display()),
        ));
    }

    // Walk, parse, validate
    let files = walk_dir(&dir)?;
    let mut all_vars = HashSet::new();
    let mut entries = Vec::new();

    for file in &files {
        let content = std::fs::read_to_string(file)
            .map_err(|e| syn::Error::new_spanned(&input, format!("{}: {e}", file.display())))?;

        let mut rel = file
            .strip_prefix(&dir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");

        if let Some(prefix) = attrs.strip_prefix.as_deref() {
            if let Some(trimmed) = rel.strip_prefix(prefix) {
                rel = String::from(trimmed);
            }
        }
        if let Some(suffix) = attrs.strip_suffix.as_deref() {
            if let Some(trimmed) = rel.strip_suffix(suffix) {
                rel = String::from(trimmed);
            }
        }

        let segs = parse_template(&content);
        let vars = collect_vars(&segs);

        // Every variable must be a valid Rust identifier
        for v in &vars {
            syn::parse_str::<Ident>(v).map_err(|_| {
                syn::Error::new_spanned(
                    &input,
                    format!("`{v}` in `{rel}` is not a valid identifier"),
                )
            })?;
        }

        // Default + Strict: template variables must exist as struct fields
        if !matches!(attrs.mode, Mode::Lenient) {
            for v in &vars {
                if !fields.contains(v) {
                    return Err(syn::Error::new_spanned(
                        &input,
                        format!("variable `{v}` in `{rel}` has no matching struct field"),
                    ));
                }
            }
        }

        all_vars.extend(vars);
        entries.push(FileEntry {
            rel,
            segs,
            abs: file.to_string_lossy().into(),
        });
    }

    // Strict: every struct field must appear in at least one template
    if matches!(attrs.mode, Mode::Strict) {
        for f in &fields {
            if !all_vars.contains(f) {
                return Err(syn::Error::new_spanned(
                    &input,
                    format!("field `{f}` unused in any template (strict mode)"),
                ));
            }
        }
    }

    // Build the render items
    let render_items = entries.iter().map(|e| {
        let rel = &e.rel;
        let has_vars = e.segs.iter().any(|s| matches!(s, Seg::Var(_)));

        let content_expr = if has_vars {
            let mut fmt = String::new();
            let mut args = Vec::<proc_macro2::TokenStream>::new();
            for seg in &e.segs {
                match seg {
                    Seg::Lit(l) => fmt.push_str(&l.replace('{', "{{").replace('}', "}}")),
                    Seg::Var(v) => {
                        fmt.push_str("{}");
                        let id = Ident::new(v, Span::call_site());
                        args.push(quote! { self.#id });
                    }
                }
            }
            quote! { ::std::borrow::Cow::Owned(format!(#fmt, #(#args),*)) }
        } else {
            let text: String = e
                .segs
                .iter()
                .map(|s| match s {
                    Seg::Lit(l) => l.as_str(),
                    _ => unreachable!(),
                })
                .collect();
            quote! { ::std::borrow::Cow::Borrowed(#text) }
        };

        quote! { (#rel, #content_expr) }
    });

    // File-dependency tracking so cargo rebuilds when templates change
    let tracking = entries.iter().map(|e| {
        let abs = &e.abs;
        quote! { let _ = include_bytes!(#abs); }
    });

    Ok(quote! {
        impl #impl_g ::textus::Template for #name #ty_g #where_cl {
            fn render(&self) -> ::std::vec::Vec<(
                &'static str,
                ::std::borrow::Cow<'static, str>,
            )> {
                #(#tracking)*
                vec![#(#render_items),*]
            }
        }
    })
}
