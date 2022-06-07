#![allow(unused)]
use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::{self, Ident, Lifetime, GenericParam, LifetimeDef, TypeParamBound, TraitBound, TraitBoundModifier, Path, PathSegment, BoundLifetimes, PathArguments, AngleBracketedGenericArguments, GenericArgument};

use crate::bound;
use crate::dummy;
use crate::fragment::{Fragment, Stmts};
use crate::internals::ast::{Container, Data, Field, Style, Variant, get_option_count};
use crate::internals::{attr, Ctxt, Derive};

use std::collections::BTreeSet;
use std::str::FromStr;
use syn::token::Token;

pub fn expand_derive_raw_decode(input: &syn::DeriveInput) -> Result<TokenStream, Vec<syn::Error>> {
    let ctxt = Ctxt::new();
    let cont: Container = match Container::from_ast(&ctxt, input, Derive::RawDecode) {
        Some(cont) => cont,
        None => return Err(ctxt.check().unwrap_err()),
    };
    ctxt.check()?;

    let ident = &cont.ident;
    let params = Parameters::new(&cont);
    let (de_impl_generics, _, ty_generics, where_clause) = split_with_de_lifetime(&params);
    let body = Stmts(decode_body(&cont, &params));
    let delife = params.borrowed.de_lifetime();
    let call_name = TokenStream::from_str((ident.to_string() + "_call").as_str()).unwrap();
    let impl_block = quote! {
        #[automatically_derived]
        #[allow(non_snake_case)]
        fn #call_name<'a, T: RawDecode<'a>>(__buf__: &'a [u8]) -> BuckyResult<(T, &'a [u8])> {
            T::raw_decode(__buf__)
        }

        #[automatically_derived]
        #[allow(non_snake_case)]
        impl #de_impl_generics RawDecode<#delife> for #ident #ty_generics #where_clause {
            fn raw_decode(__buf__: &#delife [u8]) -> BuckyResult<(Self, &#delife [u8])>
            {
                #body
            }
        }
    };

    Ok(dummy::wrap_in_const(
        impl_block,
    ))
}

struct Parameters {
    /// Name of the type the `derive` is on.
    pub local: syn::Ident,

    /// Path to the type the impl is for. Either a single `Ident` for local
    /// types or `some::remote::Ident` for remote types. Does not include
    /// generic parameters.
    this: syn::Path,

    /// Generics including any explicit and inferred bounds for the impl.
    generics: syn::Generics,

    /// Lifetimes borrowed from the deserializer. These will become bounds on
    /// the `'de` lifetime of the deserializer.
    borrowed: BorrowedLifetimes,

    /// At least one field has a serde(getter) attribute, implying that the
    /// remote type has a private field.
    has_getter: bool,
}

fn has_lifetime(generics: &syn::Generics) -> bool {
    for param in generics.params.iter() {
        if let GenericParam::Lifetime(_) = param {
            return true
        }
    }
    false
}

fn get_lifetime(generics: &syn::Generics) -> Option<&LifetimeDef> {
    for param in generics.params.iter() {
        if let GenericParam::Lifetime(def) = param {
            return Some(def)
        }
    }
    None
}

impl Parameters {
    fn new(cont: &Container) -> Self {
        let local = cont.ident.clone();
        let this = cont.ident.clone().into();
        let lifetime = get_lifetime(&cont.generics);
        let mut set = BTreeSet::<syn::Lifetime>::new();
        if lifetime.is_some() {
            set.insert(lifetime.unwrap().lifetime.clone());
        } else {
            set.insert(syn::Lifetime::new("'__de__", Span::call_site()));
        }
        let borrowed = BorrowedLifetimes::Borrowed(set);
        let generics = build_generics(cont, &borrowed);
        let has_getter = false;

        Parameters {
            local,
            this,
            generics,
            borrowed,
            has_getter,
        }
    }

    /// Type name to use in error messages and `&'static str` arguments to
    /// various Deserializer methods.
    fn type_name(&self) -> String {
        self.this.segments.last().unwrap().ident.to_string()
    }
}

// All the generics in the input, plus a bound `T: Deserialize` for each generic
// field type that will be deserialized by us, plus a bound `T: Default` for
// each generic field type that will be set to a default value.
fn build_generics(cont: &Container, _: &BorrowedLifetimes) -> syn::Generics {
    let generics = bound::without_defaults(cont.generics);

    generics
}

enum BorrowedLifetimes {
    Borrowed(BTreeSet<syn::Lifetime>),
    Static,
}

impl BorrowedLifetimes {
    fn de_lifetime(&self) -> syn::Lifetime {
        match self {
            BorrowedLifetimes::Borrowed(set) => {
                // syn::Lifetime::new("'de", Span::call_site())
                set.iter().next().unwrap().clone()
            },
            BorrowedLifetimes::Static => syn::Lifetime::new("'static", Span::call_site()),
        }
    }

    fn de_lifetime_def(&self) -> Option<syn::LifetimeDef> {
        match self {
            BorrowedLifetimes::Borrowed(bounds) => Some(syn::LifetimeDef {
                attrs: Vec::new(),
                lifetime: bounds.iter().next().unwrap().clone(),
                colon_token: None,
                bounds: Default::default(),
            }),
            BorrowedLifetimes::Static => None,
        }
    }
}

fn decode_body(cont: &Container, params: &Parameters) -> Fragment {
    match &cont.data {
        Data::Enum(variants) => decode_enum(params, variants, &cont.attrs),
        Data::Struct(Style::Struct, fields) => {
            decode_struct(None, params, fields, &cont.attrs, None, &Untagged::No)
        }
        Data::Struct(Style::Tuple, fields) | Data::Struct(Style::Newtype, fields) => {
            decode_tuple(None, params, fields, &cont.attrs, None)
        }
        Data::Struct(Style::Unit, _) => decode_unit_struct(params, &cont.attrs),
    }
}

fn decode_unit_struct(_params: &Parameters, _cattrs: &attr::Container) -> Fragment {
    let name = &_params.local;
    quote_block!(
        Ok((#name, __buf__))
    )
}

fn decode_tuple(
    _variant_ident: Option<&syn::Ident>,
    _params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
    _deserializer: Option<TokenStream>,
) -> Fragment {
    let flag = decode_option_flag(fields, cattrs.optimize_option);

    let name = &_params.local;
    let call_name = TokenStream::from_str((name.to_string() + "_call").as_str()).unwrap();

    let field_list: Vec<_> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let id = Ident::new(&format!("__field{}", i), Span::call_site());
            quote! {#id}
        })
        .collect();
    let mut option_index = 0;
    let field_decode_list: Vec<_> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let id = Ident::new(&format!("__field{}", i), Span::call_site());
            if cattrs.optimize_option && field.is_option() {
                let ty = field.get_option_type();
                let item = quote! {
                    let (#id, __buf__) = if (flag & (1 << #option_index) != 0) {
                        let (obj, __buf__): (#ty, &[u8]) = #call_name(__buf__)?;
                        (Some(obj), __buf__)
                    } else {
                        (None, __buf__)
                    };
                };
                option_index += 1;
                item
            } else if field.is_vec_u8() {
                quote! {
                    let (#id, __buf__) = {
                        let (len, __buf__) = cyfs_base::USize::raw_decode(__buf__)?;
                        let len = len.value();
                        if len == 0 {
                            (Vec::new(), __buf__)
                        } else {
                            let mut bytes_buf = Vec::<u8>::with_capacity(len as usize);
                            unsafe {
                                std::ptr::copy::<u8>(__buf__.as_ptr(),  bytes_buf.as_mut_ptr(), len as usize);
                                bytes_buf.set_len(len as usize);
                            }

                            ( bytes_buf, &__buf__[len as usize..])
                        }
                    };
                }
            } else {
                let ty = field.ty;
                quote! {let (#id, __buf__): (#ty, &[u8]) = #call_name(__buf__)?;}
            }
        })
        .collect();
    quote_block!({
        #flag
        #(#field_decode_list)*
        Ok((#name (#(#field_list),*), __buf__))
    })
}

#[cfg(feature = "deserialize_in_place")]
fn deserialize_tuple_in_place(
    variant_ident: Option<syn::Ident>,
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
    deserializer: Option<TokenStream>,
) -> Fragment {
    quote_block!({})

}

enum Untagged {
    Yes,
    No,
}

fn decode_option_flag(fields: &[Field], optimize_option: bool) -> TokenStream {
    let option_count = if optimize_option {
        get_option_count(fields)
    } else {
        0_u32
    };
    if optimize_option && option_count > 0 {
        if option_count <= 8 {
            quote! {let (flag, __buf__) = u8::raw_decode(__buf__)?;}
        } else if option_count <= 16 {
            quote! {let (flag, __buf__) = u16::raw_decode(__buf__)?;}
        } else if option_count <= 32 {
            quote! {let (flag, __buf__) = u32::raw_decode(__buf__)?;}
        } else if option_count <= 64 {
            quote! {let (flag, __buf__) = u64::raw_decode(__buf__)?;}
        } else if option_count <= 128 {
            quote! {let (flag, __buf__) = u128::raw_decode(__buf__)?;}
        } else if option_count <= 256 {
            quote! {let (flag, __buf__) = u256::raw_decode(__buf__)?;}
        } else {
            quote! {}
        }
    } else {
        quote! {}
    }
}

fn decode_struct(
    _variant_ident: Option<&syn::Ident>,
    params: &Parameters,
    fields: &[Field],
    cattr: &attr::Container,
    _deserializer: Option<TokenStream>,
    _: &Untagged,
) -> Fragment {
    let flag = decode_option_flag(fields, cattr.optimize_option);

    let name = &params.local;
    let call_name = TokenStream::from_str((name.to_string() + "_call").as_str()).unwrap();
    let mut option_index = 0;
    let list: Vec<_> = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| !field.attrs.skip_deserializing())
        .map(|(i, field)| {
            let member = &field.member;
            // let ty_str = quote! {#ty}.to_string();
            // let ty_str = TokenStream::from_str(ty_str.replace("<", "::<").as_str()).unwrap();
            if cattr.optimize_option && field.is_option() {
                let ty = field.get_option_type();
                let item = quote! {
                    let (#member, __buf__) = if (flag & (1 << #option_index) != 0) {
                        let (obj, __buf__): (#ty, &[u8]) = #call_name(__buf__)?;
                        (Some(obj), __buf__)
                    } else {
                        (None, __buf__)
                    };
                };
                option_index += 1;
                item
            } else if field.is_vec_u8() {
                let item = quote! {
                    let (#member, __buf__) = {
                        let (len, __buf__) = cyfs_base::USize::raw_decode(__buf__)?;
                        let len = len.value();
                        if len == 0 {
                            (Vec::new(), __buf__)
                        } else {
                            let mut bytes_buf = Vec::<u8>::with_capacity(len as usize);
                            unsafe {
                                std::ptr::copy::<u8>(__buf__.as_ptr(),  bytes_buf.as_mut_ptr(), len as usize);
                                bytes_buf.set_len(len as usize);
                            }

                            ( bytes_buf, &__buf__[len as usize..])
                        }
                    };
                };

                // println!("{}", item.to_string());

                item
            } else {
                let ty = field.ty;
                quote! {
                    let (#member, __buf__): (#ty, &[u8]) = #call_name(__buf__)?;
                }
            }
        })
        .collect();
    let list1: Vec<_> = fields
        .iter()
        .map(|field| {
            let member = &field.member;
            if field.attrs.skip_deserializing() {
                quote! {
                    #member: Default::default()
                }
            } else {
                quote! {
                    #member
                }
            }
        })
        .collect();
    quote_block!({
        #flag
        #(#list)*
        Ok((#name{#(#list1),*}, __buf__))
    })
}

fn decode_enum(
    params: &Parameters,
    variants: &[Variant],
    cattr: &attr::Container,
) -> Fragment {
    let enum_name = &params.local;
    if variants.len() == 0 {
        return quote_block!{unimplemented!()}
    }
    let call_name = TokenStream::from_str((enum_name.to_string() + "_call").as_str()).unwrap();
    let list: Vec<_> = variants
        .iter()
        .enumerate()
        .map(|(variant_index, variant)| {
            let index = variant_index;
            let name = &variant.ident;
            match variant.style {
                Style::Unit => {
                    quote!{
                        #index => {
                            Ok((#enum_name::#name, __buf__))
                        }
                    }
                }
                // Style::Newtype => {
                //     let flag = decode_option_flag(fields);
                //     let ty = variant.fields[0].ty;
                //     quote!{
                //         #index => {
                //             let (obj, buf): (#ty, &[u8]) = #call_name(buf)?;
                //             Ok((#enum_name::#name(obj), buf))
                //         }
                //     }
                // }
                Style::Newtype | Style::Tuple => {
                    let flag = decode_option_flag(&variant.fields, cattr.optimize_option);
                    let mut option_index = 0;
                    let items: Vec<_> = variant.fields
                        .iter()
                        .enumerate()
                        .map(|(i, field)| {
                            let field_name = format!("__field{}", i);
                            let field_indent = Ident::new(field_name.as_str(), Span::call_site());
                            if cattr.optimize_option && field.is_option() {
                                let ty = field.get_option_type();
                                let item = quote! {
                                    let (#field_indent, __buf__) = if (flag & (1 << #option_index) != 0) {
                                        let (obj, __buf__): (#ty, &[u8]) = #call_name(__buf__)?;
                                        (Some(obj), __buf__)
                                    } else {
                                        (None, __buf__)
                                    };
                                };
                                option_index += 1;
                                item
                            } else if field.is_vec_u8() {
                                quote! {
                                    let (#field_indent, __buf__) = {
                                        let (len, __buf__) = cyfs_base::USize::raw_decode(__buf__)?;
                                        let len = len.value();
                                        if len == 0 {
                                            (Vec::new(), __buf__)
                                        } else {
                                            let mut bytes_buf = Vec::<u8>::with_capacity(len as usize);
                                            unsafe {
                                                std::ptr::copy::<u8>(__buf__.as_ptr(),  bytes_buf.as_mut_ptr(), len as usize);
                                                bytes_buf.set_len(len as usize);
                                            }

                                            ( bytes_buf, &__buf__[len as usize..])
                                        }
                                    };
                                }
                            } else {
                                let ty = field.ty;
                                quote!(
                                    let (#field_indent, __buf__): (#ty, &[u8]) = #call_name(__buf__)?;
                                )
                            }
                        })
                        .collect();

                    let item_names: Vec<_> = variant.fields
                        .iter()
                        .enumerate()
                        .map(|(i, field)| {
                            let field_name = format!("__field{}", i);
                            Ident::new(field_name.as_str(), Span::call_site())
                        })
                        .collect();
                    quote!{
                        #index => {
                            #flag
                            #(#items)*
                            Ok((#enum_name::#name(#(#item_names),*), __buf__))
                        }
                    }
                }
                Style::Struct => {
                    if variant.fields.len() == 0 {
                        quote!{
                            #index => {
                                Ok((#enum_name::#name{}, __buf__))
                            }
                        }
                    } else {
                        let flag = decode_option_flag(&variant.fields, cattr.optimize_option);
                        let mut option_index = 0;
                        let items: Vec<_> = variant.fields
                            .iter()
                            .enumerate()
                            .filter(|(_, field)| !field.attrs.skip_deserializing())
                            .map(|(i, field)| {
                                let ty = field.ty;
                                let field_indent = &field.member;
                                if cattr.optimize_option && field.is_option() {
                                    let ty = field.get_option_type();
                                    let item = quote! {
                                    let (#field_indent, __buf__) = if (flag & (1 << #option_index) != 0) {
                                            let (obj, __buf__): (#ty, &[u8]) = #call_name(__buf__)?;
                                            (Some(obj), __buf__)
                                        } else {
                                            (None, __buf__)
                                        };
                                    };
                                    option_index += 1;
                                    item
                                }  else if field.is_vec_u8() {
                                    quote! {
                                        let (#field_indent, __buf__) = {
                                            let (len, __buf__) = cyfs_base::USize::raw_decode(__buf__)?;
                                            let len = len.value();
                                            if len == 0 {
                                                (Vec::new(), __buf__)
                                            } else {
                                                let mut bytes_buf = Vec::<u8>::with_capacity(len as usize);
                                                unsafe {
                                                    std::ptr::copy::<u8>(__buf__.as_ptr(),  bytes_buf.as_mut_ptr(), len as usize);
                                                    bytes_buf.set_len(len as usize);
                                                }

                                                ( bytes_buf, &__buf__[len as usize..])
                                            }
                                        };
                                    }
                                } else {
                                    quote!(
                                        let (#field_indent, __buf__): (#ty, &[u8]) = #call_name(__buf__)?;
                                    )
                                }
                            })
                            .collect();

                        let item_names: Vec<_> = variant.fields
                            .iter()
                            .map(|field| {
                                let member = &field.member;
                                if field.attrs.skip_deserializing() {
                                    quote! {
                                        #member: Default::default()
                                    }
                                } else {
                                    quote! {
                                        #member
                                    }
                                }
                            })
                            .collect();
                        quote!{
                            #index => {
                                #flag
                                #(#items)*
                                Ok((#enum_name::#name{#(#item_names),*}, __buf__))
                            }
                        }
                    }
                }
            }
        })
        .collect();
    quote_block!({
        let (element_type, __buf__) = cyfs_base::USize::raw_decode(__buf__)?;
        match element_type.value() {
            #(#list)*
            _ => {
                Err(BuckyError::new(BuckyErrorCode::NotSupport, format!("file:{} line:{} NotSupport", file!(), line!())))
            }
        }
    })
}

fn field_i(i: usize) -> Ident {
    Ident::new(&format!("__field{}", i), Span::call_site())
}

struct DeImplGenerics<'a>(&'a Parameters);
#[cfg(feature = "deserialize_in_place")]
struct InPlaceImplGenerics<'a>(&'a Parameters);

impl<'a> ToTokens for DeImplGenerics<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut cur_lifetime = None;
        let mut generics = self.0.generics.clone();
        if let Some(de_lifetime) = self.0.borrowed.de_lifetime_def() {
            cur_lifetime = Some(LifetimeDef {
                attrs: vec![],
                lifetime: Lifetime::new(format!("'{}", de_lifetime.lifetime.ident.to_string()).as_str(), Span::call_site()),
                colon_token: None,
                bounds: Default::default()
            });
            if de_lifetime.lifetime.ident.to_string() == "__de__" {
                generics.params = Some(syn::GenericParam::Lifetime(de_lifetime))
                    .into_iter()
                    .chain(generics.params)
                    .collect();
            }
        }

        let mut new_params: Punctuated<GenericParam, syn::Token![,]> = Punctuated::new();
        for mut param in &generics.params {
            if let GenericParam::Type(ref type_param) = param {
                let mut new_param = type_param.clone();
                let mut has_decode_bound = false;
                for bound in &type_param.bounds {
                    if let TypeParamBound::Trait(trait_bound) = bound {
                        for segment in &trait_bound.path.segments {
                            if segment.ident.to_string() == "RawDecode" {
                                has_decode_bound = true;
                                break;
                            }
                        }
                    }
                }
                if !has_decode_bound {
                    let mut generate_args: Punctuated<GenericArgument, syn::Token![,]> = Punctuated::new();
                    generate_args.push(GenericArgument::Lifetime(Lifetime::from(cur_lifetime.as_ref().unwrap().lifetime.clone())));
                    new_param.bounds.push(TypeParamBound::Trait(TraitBound {
                        paren_token: None,
                        modifier: TraitBoundModifier::None,
                        lifetimes: None,
                        path: Path::from(PathSegment {
                            ident: Ident::new("RawDecode", Span::call_site()),
                            arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                                colon2_token: Default::default(),
                                lt_token: Default::default(),
                                args: generate_args,
                                gt_token: Default::default()
                            }),
                        })
                    }));
                }
                new_params.push(GenericParam::Type(new_param));
            } else {
                new_params.push(param.clone());
            }
        }
        generics.params = new_params;

        let (impl_generics, _, _) = generics.split_for_impl();
        impl_generics.to_tokens(tokens);
    }
}

#[cfg(feature = "deserialize_in_place")]
impl<'a> ToTokens for InPlaceImplGenerics<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let place_lifetime = place_lifetime();
        let mut generics = self.0.generics.clone();

        // Add lifetime for `&'place mut Self, and `'a: 'place`
        for param in &mut generics.params {
            match param {
                syn::GenericParam::Lifetime(param) => {
                    param.bounds.push(place_lifetime.lifetime.clone());
                }
                syn::GenericParam::Type(param) => {
                    param.bounds.push(syn::TypeParamBound::Lifetime(
                        place_lifetime.lifetime.clone(),
                    ));
                }
                syn::GenericParam::Const(_) => {}
            }
        }
        generics.params = Some(syn::GenericParam::Lifetime(place_lifetime))
            .into_iter()
            .chain(generics.params)
            .collect();
        if let Some(de_lifetime) = self.0.borrowed.de_lifetime_def() {
            generics.params = Some(syn::GenericParam::Lifetime(de_lifetime))
                .into_iter()
                .chain(generics.params)
                .collect();
        }
        let (impl_generics, _, _) = generics.split_for_impl();
        impl_generics.to_tokens(tokens);
    }
}

#[cfg(feature = "deserialize_in_place")]
impl<'a> DeImplGenerics<'a> {
    fn in_place(self) -> InPlaceImplGenerics<'a> {
        InPlaceImplGenerics(self.0)
    }
}

struct DeTypeGenerics<'a>(&'a Parameters);
#[cfg(feature = "deserialize_in_place")]
struct InPlaceTypeGenerics<'a>(&'a Parameters);

impl<'a> ToTokens for DeTypeGenerics<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut generics = self.0.generics.clone();
        if self.0.borrowed.de_lifetime_def().is_some() {
            let def = syn::LifetimeDef {
                attrs: Vec::new(),
                lifetime: syn::Lifetime::new("'__de__", Span::call_site()),
                colon_token: None,
                bounds: Punctuated::new(),
            };
            generics.params = Some(syn::GenericParam::Lifetime(def))
                .into_iter()
                .chain(generics.params)
                .collect();
        }
        let (_, ty_generics, _) = generics.split_for_impl();
        ty_generics.to_tokens(tokens);
    }
}

#[cfg(feature = "deserialize_in_place")]
impl<'a> ToTokens for InPlaceTypeGenerics<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut generics = self.0.generics.clone();
        generics.params = Some(syn::GenericParam::Lifetime(place_lifetime()))
            .into_iter()
            .chain(generics.params)
            .collect();

        if self.0.borrowed.de_lifetime_def().is_some() {
            let def = syn::LifetimeDef {
                attrs: Vec::new(),
                lifetime: syn::Lifetime::new("'__de__", Span::call_site()),
                colon_token: None,
                bounds: Punctuated::new(),
            };
            generics.params = Some(syn::GenericParam::Lifetime(def))
                .into_iter()
                .chain(generics.params)
                .collect();
        }
        let (_, ty_generics, _) = generics.split_for_impl();
        ty_generics.to_tokens(tokens);
    }
}

#[cfg(feature = "deserialize_in_place")]
impl<'a> DeTypeGenerics<'a> {
    fn in_place(self) -> InPlaceTypeGenerics<'a> {
        InPlaceTypeGenerics(self.0)
    }
}

#[cfg(feature = "deserialize_in_place")]
fn place_lifetime() -> syn::LifetimeDef {
    syn::LifetimeDef {
        attrs: Vec::new(),
        lifetime: syn::Lifetime::new("'place", Span::call_site()),
        colon_token: None,
        bounds: Punctuated::new(),
    }
}

fn split_with_de_lifetime(
    params: &Parameters,
) -> (
    DeImplGenerics,
    DeTypeGenerics,
    syn::TypeGenerics,
    Option<&syn::WhereClause>,
) {
    let de_impl_generics = DeImplGenerics(params);
    let de_ty_generics = DeTypeGenerics(params);
    let (_, ty_generics, where_clause) = params.generics.split_for_impl();
    (de_impl_generics, de_ty_generics, ty_generics, where_clause)
}
