use proc_macro2::Ident;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use regex::Regex;
use std::str::FromStr;
use syn::spanned::Spanned;
use syn::Meta::List;
use syn::*;

pub trait FieldEx {
    fn is_option(&self) -> bool;
    fn get_option_type(&self) -> TokenStream;
    fn get_option_type_str(&self) -> String;
    fn is_vec_u8(&self) -> bool;
}

impl FieldEx for Field {
    fn is_option(&self) -> bool {
        let ty = &self.ty;
        let ty = quote!(#ty).to_string();
        let re = Regex::new(r"^Option[ ]*<").unwrap();
        re.is_match(ty.as_str())
    }

    fn get_option_type(&self) -> TokenStream {
        let ty = &self.ty;
        let ty = quote!(#ty).to_string();
        let re = Regex::new(r"^Option[ ]*<").unwrap();
        let ret = re.replace(ty.as_str(), "").to_string();
        let re = Regex::new(r">$").unwrap();
        let ret = re.replace(ret.as_str(), "").to_string();
        let ident = Ident::new(ret.trim(), Span::call_site());
        quote! {#ident}
    }

    fn get_option_type_str(&self) -> String {
        let ty = &self.ty;
        let ty = quote!(#ty).to_string();
        let re = Regex::new(r"^Option[ ]*<").unwrap();
        let ret = re.replace(ty.as_str(), "").to_string();
        let re = Regex::new(r">$").unwrap();
        let ret = re.replace(ret.as_str(), "").to_string();
        ret
    }

    fn is_vec_u8(&self) -> bool {
        let ty = &self.ty;
        let ty = quote!(#ty).to_string();
        let re = Regex::new(r"Vec[ ]*<[ ]*u8[ ]*>$").unwrap();
        re.is_match(ty.trim())
    }
}

// pub fn get_proto_type(input: &DeriveInput) -> Result<TokenStream> {
//     for attr in input.attrs.iter() {
//         if !attr.path.is_ident("cyfs") {
//             continue
//         } else {
//             match attr.parse_meta() {
//                 Ok(List(meta)) => {
//                     for nest in meta.nested.iter() {
//                         match nest {
//                             NestedMeta::Meta(meta) => {
//                                 match meta {
//                                     Meta::Path(path) => {
//                                         println!("path {}", path.to_token_stream().to_string());
//                                     },
//                                     Meta::List(meta_list) => {
//                                         println!("meta_list {}", meta_list.to_token_stream().to_string());
//                                     },
//                                     Meta::NameValue(value) => {
//                                         if value.path.is_ident("proto_type") {
//                                             if let Lit::Str(lit_str) = &value.lit {
//                                                 let ty: syn::Type = syn::parse_str(lit_str.value().as_str()).unwrap();
//                                                 return Ok(ty.to_token_stream());
//                                             }
//                                         }
//                                     }
//                                 }
//                             },
//                             NestedMeta::Lit(lit) => {
//                                 println!("lit {}", lit.to_token_stream().to_string());
//                             }
//                         }
//                     }
//                 },
//                 Ok(other) => {
//                     return Err(syn::Error::new(other.span(), format!("attribute type err.")));
//                 },
//                 Err(err) => {
//                     return Err(err);
//                 }
//             }
//         }
//     }
//     return Err(syn::Error::new(input.span(), format!("not find proto_type attribute")));
// }

pub fn get_proto_type(input: &DeriveInput) -> Result<TokenStream> {
    for attr in input.attrs.iter() {
        if !attr.path.is_ident("cyfs_protobuf_type") {
            continue;
        } else {
            match attr.parse_meta() {
                Ok(List(meta)) => {
                    for nest in meta.nested.iter() {
                        match nest {
                            NestedMeta::Meta(meta) => {
                                match meta {
                                    Meta::Path(path) => {
                                        return Ok(path.to_token_stream());
                                    }
                                    Meta::List(meta_list) => {
                                        println!(
                                            "meta_list {}",
                                            meta_list.to_token_stream().to_string()
                                        );
                                    }
                                    Meta::NameValue(value) => {
                                        println!(
                                            "NameValue {}",
                                            value.to_token_stream().to_string()
                                        );
                                        // if value.path.is_ident("proto_type") {
                                        //     if let Lit::Str(lit_str) = &value.lit {
                                        //         let ty: syn::Type = syn::parse_str(lit_str.value().as_str()).unwrap();
                                        //         return Ok(ty.to_token_stream());
                                        //     }
                                        // }
                                    }
                                }
                            }
                            NestedMeta::Lit(lit) => {
                                println!("lit {}", lit.to_token_stream().to_string());
                            }
                        }
                    }
                }
                Ok(other) => {
                    return Err(syn::Error::new(
                        other.span(),
                        format!("attribute type err."),
                    ));
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
    }
    return Err(syn::Error::new(
        input.span(),
        format!("not find cyfs_protobuf_type attribute"),
    ));
}

#[allow(unused)]
fn is_short_number(ty: &str) -> bool {
    if ty == "u8" || ty == "i8" || ty == "i16" || ty == "u16" {
        true
    } else {
        false
    }
}

#[allow(unused)]
fn is_number(ty: &str) -> bool {
    if ty == "u8"
        || ty == "i8"
        || ty == "i16"
        || ty == "u16"
        || ty == "i32"
        || ty == "u32"
        || ty == "i64"
        || ty == "u64"
        || ty == "f32"
        || ty == "f64"
        || ty == "bool"
    {
        true
    } else {
        false
    }
}

#[allow(unused)]
fn is_string(ty: &str) -> bool {
    if ty == "String" {
        true
    } else {
        false
    }
}

fn is_vec_u8(ty: &str) -> bool {
    if ty == "Vec < u8 >" {
        true
    } else {
        false
    }
}

pub fn derive_protobuf_try_from_fn_impl(
    input: syn::DeriveInput,
) -> Result<proc_macro::TokenStream> {
    let ident = &input.ident;
    let proto_type = get_proto_type(&input)?;
    let obj_try_from_proto_body = obj_try_from_proto_body(&proto_type, &input);
    let proto_try_from_obj_ref_body = proto_try_from_obj_ref_body(&proto_type, &input);
    let impl_block = quote! {
        #[automatically_derived]
        #[allow(non_snake_case)]
        impl cyfs_base::ProtobufTransform<&#ident> for #proto_type {
            fn transform(_value_: &#ident) -> BuckyResult<Self> {
                Ok(#proto_try_from_obj_ref_body)
            }
        }

        #[automatically_derived]
        #[allow(non_snake_case)]
        impl cyfs_base::ProtobufTransform<&#ident> for Option<#proto_type> {
            fn transform(_value_: &#ident) -> BuckyResult<Self> {
                Ok(Some(cyfs_base::ProtobufTransform::transform(_value_)?))
            }
        }

        #[automatically_derived]
        #[allow(non_snake_case)]
        impl cyfs_base::ProtobufTransform<#proto_type> for #ident {
            fn transform(_value_: #proto_type) -> BuckyResult<Self> {
                Ok(#obj_try_from_proto_body)
            }
        }

        #[automatically_derived]
        #[allow(non_snake_case)]
        impl cyfs_base::ProtobufTransform<Option<#proto_type>> for #ident {
            fn transform(_value_: Option<#proto_type>) -> BuckyResult<Self> {
                match _value_ {
                    Some(_value_) => {
                        Ok(cyfs_base::ProtobufTransform::transform(_value_)?)
                    },
                    None => Err(BuckyError::new(BuckyErrorCode::InvalidInput, format!("transform failed. value can't None")))
                }
            }
        }
    };
    // println!("{}", impl_block.to_string());
    Ok(impl_block.into())
}

pub fn derive_protobuf_transform_type_fn_impl(
    input: syn::DeriveInput,
) -> Result<proc_macro::TokenStream> {
    let ident = &input.ident;
    let proto_type = get_proto_type(&input)?;
    let impl_block = quote! {

        #[automatically_derived]
        #[allow(non_snake_case)]
        impl cyfs_base::ProtobufTransform<&#ident> for Option<#proto_type> {
            fn transform(_value_: &#ident) -> BuckyResult<Self> {
                Ok(Some(cyfs_base::ProtobufTransform::transform(_value_)?))
            }
        }

        #[automatically_derived]
        #[allow(non_snake_case)]
        impl cyfs_base::ProtobufTransform<Option<#proto_type>> for #ident {
            fn transform(_value_: Option<#proto_type>) -> BuckyResult<Self> {
                match _value_ {
                    Some(_value_) => {
                        Ok(cyfs_base::ProtobufTransform::transform(_value_)?)
                    },
                    None => Err(BuckyError::new(BuckyErrorCode::InvalidInput, format!("transform failed. value can't None")))
                }
            }
        }
    };
    // println!("{}", impl_block.to_string());
    Ok(impl_block.into())
}

pub fn derive_proto_encode_fn_impl(input: syn::DeriveInput) -> Result<proc_macro::TokenStream> {
    let ident = &input.ident;
    let proto_type = get_proto_type(&input)?;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let impl_block = quote! {
        #[automatically_derived]
        #[allow(non_snake_case)]
        impl #impl_generics cyfs_base::RawEncode for #ident #ty_generics #where_clause {
            fn raw_encode<'__de__>(&self, __buf__: &'__de__ mut [u8], __purpose__: &Option<RawEncodePurpose>) -> cyfs_base::BuckyResult<&'__de__ mut [u8]> {
                use prost::Message;
                use prost::bytes::BufMut;
                let proto_obj: #proto_type = cyfs_base::ProtobufTransform::transform(self)?;
                let required = proto_obj.encoded_len();
                let remaining = __buf__.len();
                if required > remaining {
                    let msg = format!("raw_encode failed.err {} except {}", remaining, required);
                    log::error!("{}", msg.as_str());
                    return Err(cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCode::OutOfLimit, msg));
                }

                let mut tmp_buf = __buf__;
                proto_obj.encode_raw(&mut tmp_buf);
                Ok(tmp_buf)
            }

            fn raw_measure(&self, __purpose__: &Option<RawEncodePurpose>) -> cyfs_base::BuckyResult<usize> {
                use prost::Message;
                let proto_obj: #proto_type = cyfs_base::ProtobufTransform::transform(self)?;
                Ok(proto_obj.encoded_len())
            }
        }
    };
    // println!("{}", impl_block.to_string());
    Ok(impl_block.into())
}

pub fn derive_proto_encode_empty_fn_impl(input: syn::DeriveInput) -> Result<proc_macro::TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let impl_block = quote! {
        #[automatically_derived]
        #[allow(non_snake_case)]
        impl #impl_generics cyfs_base::RawEncode for #ident #ty_generics #where_clause {
            fn raw_encode<'__de__>(&self, __buf__: &'__de__ mut [u8], __purpose__: &Option<RawEncodePurpose>) -> cyfs_base::BuckyResult<&'__de__ mut [u8]> {
                Ok(__buf__)
            }

            fn raw_measure(&self, __purpose__: &Option<RawEncodePurpose>) -> cyfs_base::BuckyResult<usize> {
                Ok(0)
            }
        }
    };
    // println!("{}", impl_block.to_string());
    Ok(impl_block.into())
}

pub fn derive_proto_decode_fn_impl(input: syn::DeriveInput) -> Result<proc_macro::TokenStream> {
    let ident = &input.ident;
    let proto_type = get_proto_type(&input)?;
    let mut generics = input.generics.clone();
    generics.params.insert(
        0,
        GenericParam::Lifetime(LifetimeDef::new(Lifetime::new(
            "'__de__",
            Span::call_site(),
        ))),
    );
    let (_, ty_generics, where_clause) = input.generics.split_for_impl();
    let (de_impl_generics, _, _) = generics.split_for_impl();
    let impl_block = quote! {
        #[automatically_derived]
        #[allow(non_snake_case)]
        impl #de_impl_generics cyfs_base::RawDecode<'__de__> for #ident #ty_generics #where_clause {
            fn raw_decode(__buf__: &'__de__ [u8]) -> cyfs_base::BuckyResult<(Self, &'__de__ [u8])>
            {
                use prost::Message;
                let t = #proto_type::decode(&mut std::io::Cursor::new(__buf__)).map_err(|e| {
                        let msg = format!("raw_encode failed.err {}", e);
                        log::error!("{}", msg.as_str());
                        cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCode::Failed, msg)
                    })?;
                let len = t.encoded_len();
                Ok((cyfs_base::ProtobufTransform::transform(t)?, &__buf__[len..]))
            }
        }
    };
    // println!("{}", impl_block.to_string());
    Ok(impl_block.into())
}

pub fn derive_proto_decode_empty_fn_impl(input: syn::DeriveInput) -> Result<proc_macro::TokenStream> {
    let ident = &input.ident;
    let mut generics = input.generics.clone();
    generics.params.insert(
        0,
        GenericParam::Lifetime(LifetimeDef::new(Lifetime::new(
            "'__de__",
            Span::call_site(),
        ))),
    );
    let (_, ty_generics, where_clause) = input.generics.split_for_impl();
    let (de_impl_generics, _, _) = generics.split_for_impl();
    let impl_block = quote! {
        #[automatically_derived]
        #[allow(non_snake_case)]
        impl #de_impl_generics cyfs_base::RawDecode<'__de__> for #ident #ty_generics #where_clause {
            fn raw_decode(__buf__: &'__de__ [u8]) -> cyfs_base::BuckyResult<(Self, &'__de__ [u8])>
            {
                Ok((Self::default(), __buf__))
            }
        }
    };
    // println!("{}", impl_block.to_string());
    Ok(impl_block.into())
}

pub fn obj_try_from_proto_body(proto_type: &TokenStream, input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    match &input.data {
        Data::Enum(variants) => obj_try_from_proto_encode_enum(proto_type, ident, variants),
        Data::Struct(data_st) => obj_try_from_proto_encode_struct(proto_type, ident, data_st),
        Data::Union(data_union) => obj_try_from_proto_encode_union(data_union),
    }
}

fn obj_try_from_proto_encode_enum(
    proto_type: &TokenStream,
    enum_name: &Ident,
    variants: &DataEnum,
) -> TokenStream {
    let mut token_list: Vec<TokenStream> = variants.variants.iter().enumerate().map(|(enum_member_index, variant)| {
        let enum_index = TokenStream::from_str(format!("{}", enum_member_index).as_str()).unwrap();
        let var_ident = &variant.ident;

        let stmt = if variant.fields.len() == 0 {
            if proto_type.to_string() == "i32".to_string() {
                quote! {
                    #enum_index => #enum_name::#var_ident,
                }
            } else {
                quote! {
                    #proto_type::#var_ident => #enum_name::#var_ident,
                }
            }
        } else {
            let params_tokens: Vec<TokenStream> = variant.fields.iter().enumerate().map(|(index, field)| {
                let field_ident = &field.ident;
                if field_ident.is_some() {
                    let field_ident = field_ident.as_ref().unwrap();
                    quote! {#field_ident,}
                } else {
                    let field_ident = TokenStream::from_str(format!("field{}", index).as_str()).unwrap();
                    quote! {
                        #field_ident,
                    }
                }
            }).collect();

            let mut is_union = false;
            let field_tokens: Vec<TokenStream> = if variant.fields.len() == 1 {
                variant.fields.iter().enumerate().map(|(index, field)| {
                    let ident: &Option<Ident> = &field.ident;
                    let ty = &field.ty;
                    let ty_str = ty.to_token_stream().to_string();
                    if ident.is_some() {
                        is_union = false;
                        let ident = ident.as_ref().unwrap();
                        if is_vec_u8(ty_str.as_str()) {
                            quote! {
                                #ident,
                            }
                        } else {
                            quote! {
                                #ident: cyfs_base::ProtobufTransform::transform(#ident)?,
                            }
                        }
                    } else {
                        is_union = true;
                        let obj_field_ident = TokenStream::from_str(format!("field{}", index).as_str()).unwrap();
                        if is_vec_u8(ty_str.as_str()) {
                            quote! {
                                #obj_field_ident,
                            }
                        } else {
                            quote! {
                                cyfs_base::ProtobufTransform::transform(#obj_field_ident)?,
                            }
                        }
                    }
                }).collect()
            } else {
                panic!("enum {} member fields count can't more than 1", enum_name.to_string());
            };

            if is_union {
                quote! {
                    #proto_type::#var_ident(#(#params_tokens)*) => {
                        #enum_name::#var_ident(#(#field_tokens)*)
                    },
                }
            } else {
                quote! {
                    #proto_type::#var_ident(#(#params_tokens)*) => {
                        #enum_name::#var_ident{#(#field_tokens)*}
                    },
                }
            }
        };
        stmt
    }).collect();

    if proto_type.to_string() == "i32".to_string() {
        token_list.push(quote! {
                    _ => return Err(BuckyError::new(BuckyErrorCode::InvalidInput, format!("transform {} to {} failed.", _value_, stringify!(#enum_name))))
                })
    }
    quote! {
        {
            match _value_ {
                #(#token_list)*
            }
        }
    }
}

fn obj_try_from_proto_encode_struct(
    _proto_type: &TokenStream,
    struct_ident: &Ident,
    data_st: &DataStruct,
) -> TokenStream {
    let mut is_union = false;
    let token_list: Vec<TokenStream> = data_st
        .fields
        .iter()
        .enumerate()
        .map(|(_index, field)| {
            let ident: &Option<Ident> = &field.ident;
            let ty = &field.ty;
            let ty_str = ty.to_token_stream().to_string();
            if ident.is_some() {
                is_union = false;
                let ident = ident.as_ref().unwrap();
                if is_vec_u8(ty_str.as_str()) {
                    quote! {
                    #ident: _value_.#ident,
                }
                } else {
                    quote! {
                    #ident: cyfs_base::ProtobufTransform::transform(_value_.#ident)?,
                }
                }
            } else {
                panic!("struct {} member must has token name", struct_ident.to_string());
                // is_union = true;
                // let ident = Ident::new(format!("field{}", index + 1).as_str(), Span::call_site());
                // if is_vec_u8(ty_str.as_str()) {
                //     quote! {
                //     _value_.#ident,
                // }
                // } else {
                //     quote! {
                //     cyfs_base::ProtobufTransform::transform(_value_.#ident)?,
                // }
                // }
            }
        })
        .collect();
    if is_union {
        quote! {
            Self(#(#token_list)*)
        }
    } else {
        quote! {
            Self{#(#token_list)*}
        }
    }
}

#[allow(unused)]
pub fn proto_try_from_obj_body(proto_type: &TokenStream, input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    match &input.data {
        Data::Enum(variants) => proto_try_from_obj_encode_enum(proto_type, ident, variants),
        Data::Struct(data_st) => proto_try_from_obj_encode_struct(proto_type, ident, data_st),
        Data::Union(data_union) => proto_try_from_obj_encode_union(data_union),
    }
}

#[allow(unused)]
fn proto_try_from_obj_encode_enum(
    proto_type: &TokenStream,
    enum_name: &Ident,
    variants: &DataEnum,
) -> TokenStream {
    let token_list: Vec<TokenStream> = variants.variants.iter().enumerate().map(|(enum_member_index, variant)| {
        let enum_index = TokenStream::from_str(format!("{}", enum_member_index).as_str()).unwrap();
        let var_ident = &variant.ident;
        if variant.fields.len() == 0 {
            if proto_type.to_string() == "i32".to_string() {
                quote! {
                    #enum_name::#var_ident => {
                        #enum_index
                    }
                }
            } else {
                quote! {
                    #enum_name::#var_ident => {
                        #proto_type::#var_ident
                    }
                }
            }
        } else {
            let param_tokens: Vec<TokenStream> = variant.fields.iter().enumerate().map(|(index, field)| {
                let field_ident = &field.ident;
                if field_ident.is_some() {
                    let field_ident = field_ident.as_ref().unwrap();
                    quote! {#field_ident,}
                } else {
                    let field_ident = TokenStream::from_str(format!("field{}", index).as_str()).unwrap();
                    quote! {
                        #field_ident,
                    }
                }
            }).collect();

            let mut is_union = false;
            let field_set_tokens: Vec<TokenStream> = if variant.fields.len() == 1 {
                variant.fields.iter().enumerate().map(|(index, field)| {
                    let field_ident = &field.ident;
                    let ty = &field.ty;
                    let ty_str = ty.to_token_stream().to_string();
                    if field_ident.is_some() {
                        is_union = false;
                        let field_ident = field_ident.as_ref().unwrap();
                        if is_vec_u8(ty_str.as_str()) {
                            quote! {
                                #field_ident,
                            }
                        } else {
                            quote! {
                                #field_ident: cyfs_base::ProtobufTransform::transform(#field_ident)?,
                            }
                        }
                    } else {
                        is_union = true;
                        let obj_field_ident = TokenStream::from_str(format!("field{}", index).as_str()).unwrap();
                        if is_vec_u8(ty_str.as_str()) {
                            quote! {
                                #obj_field_ident,
                            }
                        } else {
                            quote! {
                                cyfs_base::ProtobufTransform::transform(#obj_field_ident)?,
                            }
                        }
                    }
                }).collect()
            } else {
                panic!("enum {} fields count can't more than 1", enum_name.to_string());
            };
            if is_union {
                quote! {
                    #enum_name::#var_ident(#(#param_tokens)*) => {
                        #proto_type::#var_ident(#(#field_set_tokens)*)
                    },
                }
            } else {
                quote! {
                    #enum_name::#var_ident {#(#param_tokens)*} => {
                        #proto_type::#var_ident{#(#field_set_tokens)*}
                    },
                }
            }
        }
    }).collect();
    quote! {
        {
            match _value_ {
                #(#token_list)*
            }
        }
    }
}

#[allow(unused)]
fn proto_try_from_obj_encode_struct(
    proto_type: &TokenStream,
    _struct_ident: &Ident,
    data_st: &DataStruct,
) -> TokenStream {
    let token_list: Vec<TokenStream> = data_st
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let ident: &Option<Ident> = &field.ident;
            let ty = &field.ty;
            let ty_str = ty.to_token_stream().to_string();
            if ident.is_some() {
                let ident = ident.as_ref().unwrap();
                if is_vec_u8(ty_str.as_str()) {
                    quote! {
                    #ident: _value_.#ident,
                }
                } else {
                    quote! {
                    #ident: cyfs_base::ProtobufTransform::transform(_value_.#ident)?,
                }
                }
            } else {
                let ident = Ident::new(format!("field{}", index + 1).as_str(), Span::call_site());
                let obj_field_ident = TokenStream::from_str(format!("{}", index).as_str()).unwrap();
                if is_vec_u8(ty_str.as_str()) {
                    quote! {
                    #ident: _value_.#obj_field_ident,
                }
                } else {
                    quote! {
                    #ident: cyfs_base::ProtobufTransform::transform(_value_.#obj_field_ident)?,
                }
                }
            }
        })
        .collect();
    quote! {
        #proto_type{#(#token_list)*}
    }
}

#[allow(unused)]
fn obj_try_from_proto_encode_union(_data_union: &DataUnion) -> TokenStream {
    panic!("unsupport union");
}

fn proto_try_from_obj_encode_union(_data_union: &DataUnion) -> TokenStream {
    panic!("unsupport union");
}

pub fn proto_try_from_obj_ref_body(proto_type: &TokenStream, input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    match &input.data {
        Data::Enum(variants) => proto_try_from_obj_ref_encode_enum(proto_type, ident, variants),
        Data::Struct(data_st) => proto_try_from_obj_ref_encode_struct(proto_type, ident, data_st),
        Data::Union(data_union) => proto_try_from_obj_ref_encode_union(data_union),
    }
}

fn proto_try_from_obj_ref_encode_enum(
    proto_type: &TokenStream,
    enum_name: &Ident,
    variants: &DataEnum,
) -> TokenStream {
    let token_list: Vec<TokenStream> = variants.variants.iter().enumerate().map(|(enum_member_index, variant)| {
        let enum_index = TokenStream::from_str(format!("{}", enum_member_index).as_str()).unwrap();
        let var_ident = &variant.ident;
        if variant.fields.len() == 0 {
            if proto_type.to_string() == "i32".to_string() {
                quote! {
                    #enum_name::#var_ident => {
                        #enum_index
                    }
                }
            } else {
                quote! {
                    #enum_name::#var_ident => {
                        #proto_type::#var_ident
                    }
                }
            }
        } else {
            let param_tokens: Vec<TokenStream> = variant.fields.iter().enumerate().map(|(index, field)| {
                let field_ident = &field.ident;
                if field_ident.is_some() {
                    let field_ident = field_ident.as_ref().unwrap();
                    quote! {#field_ident,}
                } else {
                    let param_ident = TokenStream::from_str(format!("field{}", index).as_str()).unwrap();
                    quote! {
                        #param_ident,
                    }
                }
            }).collect();

            let mut is_union = false;
            let field_set_tokens: Vec<TokenStream> = if variant.fields.len() == 1 {
                variant.fields.iter().enumerate().map(|(index, field)| {
                    let field_ident = &field.ident;
                    let ty = &field.ty;
                    let ty_str = ty.to_token_stream().to_string();
                    if field_ident.is_some() {
                        is_union = false;
                        let field_ident = field_ident.as_ref().unwrap();
                        if is_vec_u8(ty_str.as_str()) {
                            quote! {
                                #field_ident,
                            }
                        } else {
                            quote! {
                                #field_ident: cyfs_base::ProtobufTransform::transform(#field_ident)?,
                            }
                        }
                    } else {
                        is_union = true;
                        let obj_field_ident = TokenStream::from_str(format!("field{}", index).as_str()).unwrap();
                        if is_vec_u8(ty_str.as_str()) {
                            quote! {
                                #obj_field_ident,
                            }
                        } else {
                            quote! {
                                cyfs_base::ProtobufTransform::transform(#obj_field_ident)?,
                            }
                        }
                    }
                }).collect()
            } else {
                panic!("enum {} fields count can't more than 1", enum_name.to_string());
            };
            if is_union {
                quote! {
                    #enum_name::#var_ident(#(#param_tokens)*) => {
                        #proto_type::#var_ident(#(#field_set_tokens)*)
                    },
                }
            } else {
                quote! {
                    #enum_name::#var_ident {#(#param_tokens)*} => {
                        #proto_type::#var_ident{#(#field_set_tokens)*}
                    },
                }
            }
        }
    }).collect();
    if token_list.len() == 0 {
        quote! {
        {
            match _value_ {
                _ => {}
            }
        }
    }
    } else {
        quote! {
        {
            match _value_ {
                #(#token_list)*
            }
        }
    }
    }
}

fn proto_try_from_obj_ref_encode_struct(
    proto_type: &TokenStream,
    _struct_ident: &Ident,
    data_st: &DataStruct,
) -> TokenStream {
    let token_list: Vec<TokenStream> = data_st
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let ident: &Option<Ident> = &field.ident;
            let ty = &field.ty;
            let ty_str = ty.to_token_stream().to_string();
            if ident.is_some() {
                let ident = ident.as_ref().unwrap();
                if is_vec_u8(ty_str.as_str()) {
                    quote! {
                    #ident: _value_.#ident.clone(),
                }
                } else {
                    quote! {
                    #ident: cyfs_base::ProtobufTransform::transform(&_value_.#ident)?,
                }
                }
            } else {
                let ident = Ident::new(format!("field{}", index + 1).as_str(), Span::call_site());
                let obj_field_ident = TokenStream::from_str(format!("{}", index).as_str()).unwrap();
                if is_vec_u8(ty_str.as_str()) {
                    quote! {
                    #ident: _value_.#obj_field_ident.clone(),
                }
                } else {
                    quote! {
                    #ident: cyfs_base::ProtobufTransform::transform(&_value_.#obj_field_ident)?,
                }
                }
            }
        })
        .collect();
    quote! {
        #proto_type{#(#token_list)*}
    }
}

fn proto_try_from_obj_ref_encode_union(_data_union: &DataUnion) -> TokenStream {
    panic!("unsupport union");
}
