//! `#[derive(Template)]` — parses `{{ var }}` placeholders and substitutes
//! struct fields into them.

use proc_macro2::Span;
use quote::quote;
use std::collections::HashSet;
use syn::{DeriveInput, Ident};

use crate::{parse_attrs, rel_path, resolve_dir, walk_dir};

enum Seg {
    Lit(String),
    Var(String),
}

struct FileEntry {
    rel: String,
    segs: Vec<Seg>,
    abs: String,
}

impl FileEntry {
    fn has_vars(&self) -> bool {
        self.segs.iter().any(|s| matches!(s, Seg::Var(_)))
    }
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

// ── Code generation ──────────────────────────────────────────────────

pub(crate) fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let attrs = parse_attrs(&input, "template")?;
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

    let dir = resolve_dir(&attrs, &input)?;

    // Walk, parse, validate
    let files = walk_dir(&dir)?;
    let mut all_vars = HashSet::new();
    let mut entries = Vec::new();

    for file in &files {
        let content = std::fs::read_to_string(file)
            .map_err(|e| syn::Error::new_spanned(&input, format!("{}: {e}", file.display())))?;

        let rel = rel_path(file, &dir, &attrs);

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

        for v in &vars {
            if !fields.contains(v) {
                return Err(syn::Error::new_spanned(
                    &input,
                    format!("variable `{v}` in `{rel}` has no matching struct field"),
                ));
            }
        }

        all_vars.extend(vars);
        entries.push(FileEntry {
            rel,
            segs,
            abs: file.to_string_lossy().into(),
        });
    }

    // Every struct field must appear in at least one template
    for f in &fields {
        if !all_vars.contains(f) {
            return Err(syn::Error::new_spanned(
                &input,
                format!("field `{f}` unused in any template"),
            ));
        }
    }

    // Build the render items
    let root = &attrs.root;
    let render_items = entries.iter().map(|e| {
        let rel = &e.rel;
        let abs = &e.abs;

        let content_expr = if e.has_vars() {
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
            quote! {
                #root::__private::Cow::Owned(
                    #root::__private::format!(#fmt, #(#args),*)
                )
            }
        } else {
            quote! { #root::__private::Cow::Borrowed(include_str!(#abs)) }
        };

        quote! { (#rel, #content_expr) }
    });

    // File-dependency tracking so cargo rebuilds when templates change.
    // Variable-free files are tracked by their own `include_str!`.
    let tracking = entries.iter().filter(|e| e.has_vars()).map(|e| {
        let abs = &e.abs;
        quote! { let _ = include_bytes!(#abs); }
    });

    Ok(quote! {
        impl #impl_g #root::Template for #name #ty_g #where_cl {
            fn render(&self) -> #root::__private::Vec<(
                &'static str,
                #root::__private::Cow<'static, str>,
            )> {
                #(#tracking)*
                #root::__private::Vec::from([#(#render_items),*])
            }
        }
    })
}
