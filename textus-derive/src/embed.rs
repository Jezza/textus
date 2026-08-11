//! `#[derive(Embed)]` — embeds a directory of files verbatim as raw bytes.

use quote::quote;
use syn::DeriveInput;

use crate::{parse_attrs, rel_path, resolve_dir, walk_dir};

pub(crate) fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let attrs = parse_attrs(&input, "embed")?;
    let name = &input.ident;
    let (impl_g, ty_g, where_cl) = input.generics.split_for_impl();

    let dir = resolve_dir(&attrs, &input)?;

    // Contents are embedded untouched, so nothing is read or validated here —
    // any struct shape works, and files needn't be valid UTF-8.
    // `include_bytes!` also tracks each file, so cargo rebuilds when they change.
    let entries = walk_dir(&dir)?.into_iter().map(|file| {
        let rel = rel_path(&file, &dir, &attrs);
        let abs = file.to_string_lossy().into_owned();
        quote! { (#rel, include_bytes!(#abs)) }
    });

    let root = &attrs.root;
    Ok(quote! {
        impl #impl_g #root::Embed for #name #ty_g #where_cl {
            fn iter() -> &'static [(&'static str, &'static [u8])] {
                &[#(#entries),*]
            }
        }
    })
}
