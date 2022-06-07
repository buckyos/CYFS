//! A Serde ast, parsed from the Syn ast and ready to generate Rust code.

use crate::internals::attr;
use crate::internals::{Ctxt, Derive};
use syn;
use syn::punctuated::Punctuated;
use regex::Regex;
use proc_macro2::{TokenStream, Ident};
use proc_macro2::{Span};

/// A source data structure annotated with `#[derive(RawEncode)]` and/or `#[derive(RawDecode)]`,
/// parsed into an internal representation.
pub struct Container<'a> {
    /// The struct or enum name (without generics).
    pub ident: syn::Ident,
    /// Attributes on the structure, parsed for Serde.
    pub attrs: attr::Container,
    /// The contents of the struct or enum.
    pub data: Data<'a>,
    /// Any generics on the struct or enum.
    pub generics: &'a syn::Generics,
    /// Original input.
    pub original: &'a syn::DeriveInput,
}

/// The fields of a struct or enum.
///
/// Analagous to `syn::Data`.
pub enum Data<'a> {
    Enum(Vec<Variant<'a>>),
    Struct(Style, Vec<Field<'a>>),
}

/// A variant of an enum.
pub struct Variant<'a> {
    pub ident: syn::Ident,
    pub attrs: attr::Variant,
    pub style: Style,
    pub fields: Vec<Field<'a>>,
    pub original: &'a syn::Variant,
}

pub fn get_option_count(fields: &[Field]) -> u32 {
    let mut count = 0u32;
    for field in fields {
        if field.is_option() {
            count += 1;
        }
    }
    count
}

/// A field of a struct.
pub struct Field<'a> {
    pub member: syn::Member,
    pub attrs: attr::Field,
    pub ty: &'a syn::Type,
    pub original: &'a syn::Field,
}

impl <'a> Field<'_> {
    pub fn is_option(&self) -> bool {
        let ty = self.ty;
        let ty = quote!(#ty).to_string();
        let re = Regex::new(r"^Option[ ]*<").unwrap();
        re.is_match(ty.as_str())
    }

    pub fn get_option_type(&self) -> TokenStream {
        let ty = self.ty;
        let ty = quote!(#ty).to_string();
        let re = Regex::new(r"^Option[ ]*<").unwrap();
        let ret = re.replace(ty.as_str(), "").to_string();
        let re = Regex::new(r">$").unwrap();
        let ret = re.replace(ret.as_str(), "").to_string();
        let ident = Ident::new(ret.trim(), Span::call_site());
        quote! {#ident}
    }

    pub fn is_vec_u8(&self) -> bool {
        let ty = self.ty;
        let ty = quote!(#ty).to_string();
        let re = Regex::new(r"Vec[ ]*<[ ]*u8[ ]*>$").unwrap();
        re.is_match(ty.trim())
    }
}

#[derive(Copy, Clone)]
pub enum Style {
    /// Named fields.
    /// struct Test{ i: u8, j: u32 }
    Struct,
    /// Many unnamed fields.
    /// struct Test();
    /// struct Test(u8,u8);
    Tuple,
    /// One unnamed field.
    /// struct Test(u8);
    Newtype,
    /// No fields.
    /// struct Test;
    Unit,
}

impl<'a> Container<'a> {
    /// Convert the raw Syn ast into a parsed container object, collecting errors in `cx`.
    pub fn from_ast(
        cx: &Ctxt,
        item: &'a syn::DeriveInput,
        _: Derive,
    ) -> Option<Container<'a>> {
        let attrs = attr::Container::from_ast(cx, item);

        let data = match &item.data {
            syn::Data::Enum(data) => Data::Enum(enum_from_ast(cx, &data.variants)),
            syn::Data::Struct(data) => {
                let (style, fields) = struct_from_ast(cx, &data.fields, None);
                Data::Struct(style, fields)
            }
            syn::Data::Union(_) => {
                cx.error_spanned_by(item, "cyfs does not support derive for unions");
                return None;
            }
        };

        let item = Container {
            ident: item.ident.clone(),
            attrs,
            data,
            generics: &item.generics,
            original: item,
        };
        Some(item)
    }
}
//
// impl<'a> Data<'a> {
//     pub fn all_fields(&'a self) -> Box<dyn Iterator<Item=&'a Field<'a>> + 'a> {
//         match self {
//             Data::Enum(variants) => {
//                 Box::new(variants.iter().flat_map(|variant| variant.fields.iter()))
//             }
//             Data::Struct(_, fields) => Box::new(fields.iter()),
//         }
//     }
//
//     pub fn has_getter(&self) -> bool {
//         false
//     }
// }

fn enum_from_ast<'a>(
    cx: &Ctxt,
    variants: &'a Punctuated<syn::Variant, syn::Token![,]>,
) -> Vec<Variant<'a>> {
    variants
        .iter()
        .map(|variant| {
            let attrs = attr::Variant::from_ast(cx, variant);
            let (style, fields) =
                struct_from_ast(cx, &variant.fields, Some(&attrs));
            Variant {
                ident: variant.ident.clone(),
                attrs,
                style,
                fields,
                original: variant,
            }
        })
        .collect()
}

fn struct_from_ast<'a>(
    cx: &Ctxt,
    fields: &'a syn::Fields,
    attrs: Option<&attr::Variant>,
) -> (Style, Vec<Field<'a>>) {
    match fields {
        syn::Fields::Named(fields) => (
            Style::Struct,
            fields_from_ast(cx, &fields.named, attrs),
        ),
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => (
            Style::Newtype,
            fields_from_ast(cx, &fields.unnamed, attrs),
        ),
        syn::Fields::Unnamed(fields) => (
            Style::Tuple,
            fields_from_ast(cx, &fields.unnamed, attrs),
        ),
        syn::Fields::Unit => (Style::Unit, Vec::new()),
    }
}

fn fields_from_ast<'a>(
    cx: &Ctxt,
    fields: &'a Punctuated<syn::Field, syn::Token![,]>,
    attrs: Option<&attr::Variant>,
) -> Vec<Field<'a>> {
    fields
        .iter()
        .enumerate()
        .map(|(i, field)| Field {
            member: match &field.ident {
                Some(ident) => syn::Member::Named(ident.clone()),
                None => syn::Member::Unnamed(i.into()),
            },
            attrs: attr::Field::from_ast(cx, i, field, attrs),
            ty: &field.ty,
            original: field,
        })
        .collect()
}
