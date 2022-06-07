extern crate proc_macro;
extern crate proc_macro2;

#[macro_use]
extern crate quote;

use quote::*;
use proc_macro::TokenStream;
use crate::protobuf_codec::*;

mod internals;
#[macro_use]
mod bound;
#[macro_use]
mod fragment;

mod de;
mod dummy;
mod pretend;
mod en;
mod try_;
mod protobuf_codec;

#[proc_macro_derive(RawEncode, attributes(cyfs))]
pub fn derive_raw_encode_fn(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    en::expand_derive_raw_encode(&input)
        .unwrap_or_else(to_compile_errors)
        .into()
}

#[proc_macro_derive(RawDecode, attributes(cyfs))]
pub fn derive_raw_decode_fn(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    de::expand_derive_raw_decode(&input)
        .unwrap_or_else(to_compile_errors)
        .into()
}

#[proc_macro_derive(ProtobufTransform, attributes(cyfs_protobuf_type))]
pub fn derive_protobuf_try_from_fn(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_protobuf_try_from_fn_impl(input).unwrap_or_else(|err| {
        err.to_compile_error().into()
    })
}

#[proc_macro_derive(ProtobufTransformType, attributes(cyfs_protobuf_type))]
pub fn derive_protobuf_type_fn(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_protobuf_transform_type_fn_impl(input).unwrap_or_else(|err| {
        err.to_compile_error().into()
    })
}

#[proc_macro_derive(ProtobufEncode, attributes(cyfs_protobuf_type))]
pub fn derive_proto_encode_fn(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_proto_encode_fn_impl(input).unwrap_or_else(|err| {
        err.to_compile_error().into()
    })
}

#[proc_macro_derive(ProtobufDecode, attributes(cyfs_protobuf_type))]
pub fn derive_proto_decode_fn(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_proto_decode_fn_impl(input).unwrap_or_else(|err| {
        err.to_compile_error().into()
    })
}

#[proc_macro_derive(ProtobufEmptyEncode)]
pub fn derive_proto_encode_empty_fn(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_proto_encode_empty_fn_impl(input).unwrap_or_else(|err| {
        err.to_compile_error().into()
    })
}

#[proc_macro_derive(ProtobufEmptyDecode)]
pub fn derive_proto_decode_empty_fn(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_proto_decode_empty_fn_impl(input).unwrap_or_else(|err| {
        err.to_compile_error().into()
    })
}

fn to_compile_errors(errors: Vec<syn::Error>) -> proc_macro2::TokenStream {
    let compile_errors = errors.iter().map(syn::Error::to_compile_error);
    quote!(#(#compile_errors)*)
}
//
// #[cfg(test)]
// mod test {
//     use proc_macro::TokenStream;
//     use std::str::FromStr;
//
//     #[test]
//     fn test() {
//         let input = TokenStream::from_str(r#"struct Test {
//         t: u32,
//     }"#);
//
//         // derive_raw_enccode_fn(input);
//     }
//
// }
