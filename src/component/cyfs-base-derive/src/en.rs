#![allow(unused)]

use proc_macro2::{Span, TokenStream};
use syn::{
    self, GenericArgument, GenericParam, Ident, Index, Member, Path, PathArguments, PathSegment,
    TraitBound, TraitBoundModifier, TypeParamBound,
};

use crate::bound;
use crate::dummy;
use crate::fragment::{Fragment, Stmts};
use crate::internals::ast::{get_option_count, Container, Data, Field, Style, Variant};
use crate::internals::{attr, Ctxt, Derive};
use syn::punctuated::Punctuated;

pub fn expand_derive_raw_encode(input: &syn::DeriveInput) -> Result<TokenStream, Vec<syn::Error>> {
    let ctxt = Ctxt::new();
    let cont: Container = match Container::from_ast(&ctxt, input, Derive::RawEncode) {
        Some(cont) => cont,
        None => return Err(ctxt.check().unwrap_err()),
    };
    ctxt.check()?;

    let ident = &cont.ident;
    let mut params = Parameters::new(&cont);
    params.generics.params = add_raw_encode_bound(&params.generics.params);
    let (impl_generics, ty_generics, where_clause) = params.generics.split_for_impl();
    let (f1, f2) = raw_encode_body(&cont, &params);
    let (body, measure_body) = (Stmts(f1), Stmts(f2));

    let impl_block = quote! {
        #[automatically_derived]
        #[allow(non_snake_case)]
        impl #impl_generics RawEncode for #ident #ty_generics #where_clause {
            fn raw_encode<'__de__>(&self, __buf__: &'__de__ mut [u8], __purpose__: &Option<RawEncodePurpose>) -> BuckyResult<&'__de__ mut [u8]>
            {
                #body
            }

            fn raw_measure(&self, __purpose__: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
                #measure_body
            }
        };

    };

    Ok(dummy::wrap_in_const(impl_block))
}

fn add_raw_encode_bound(
    params: &Punctuated<GenericParam, syn::Token![,]>,
) -> Punctuated<GenericParam, syn::Token![,]> {
    let mut new_params: Punctuated<GenericParam, syn::Token![,]> = Punctuated::new();
    for mut param in params {
        if let GenericParam::Type(ref type_param) = param {
            let mut new_param = type_param.clone();
            let mut has_encode_bound = false;
            for bound in &type_param.bounds {
                if let TypeParamBound::Trait(trait_bound) = bound {
                    for segment in &trait_bound.path.segments {
                        if segment.ident.to_string() == "RawEncode" {
                            has_encode_bound = true;
                            break;
                        }
                    }
                }
            }
            if !has_encode_bound {
                new_param.bounds.push(TypeParamBound::Trait(TraitBound {
                    paren_token: None,
                    modifier: TraitBoundModifier::None,
                    lifetimes: None,
                    path: Path::from(PathSegment {
                        ident: Ident::new("RawEncode", Span::call_site()),
                        arguments: PathArguments::None,
                    }),
                }));
            }
            new_params.push(GenericParam::Type(new_param));
        } else {
            new_params.push(param.clone());
        }
    }
    new_params
}

struct Parameters {
    /// Variable holding the value being serialized. Either `self` for local
    /// types or `__self` for remote types.
    self_var: Ident,

    /// Path to the type the impl is for. Either a single `Ident` for local
    /// types or `some::remote::Ident` for remote types. Does not include
    /// generic parameters.
    this: syn::Path,

    /// Generics including any explicit and inferred bounds for the impl.
    generics: syn::Generics,

    /// Type has a `serde(remote = "...")` attribute.
    is_remote: bool,

    /// Type has a repr(packed) attribute.
    is_packed: bool,
}

impl Parameters {
    fn new(cont: &Container) -> Self {
        let self_var = Ident::new("self", Span::call_site());

        let this = cont.ident.clone().into();
        let generics = build_generics(cont);

        Parameters {
            self_var,
            this,
            generics,
            is_remote: false,
            is_packed: false,
        }
    }

    /// Type name to use in error messages and `&'static str` arguments to
    /// various Serializer methods.
    fn type_name(&self) -> String {
        self.this.segments.last().unwrap().ident.to_string()
    }
}

// All the generics in the input, plus a bound `T: Serialize` for each generic
// field type that will be serialized by us.
fn build_generics(cont: &Container) -> syn::Generics {
    let generics = bound::without_defaults(cont.generics);
    generics
}

fn raw_encode_body(cont: &Container, params: &Parameters) -> (Fragment, Fragment) {
    match &cont.data {
        Data::Enum(variants) => encode_enum(params, variants, cont.attrs.optimize_option),
        Data::Struct(Style::Struct, fields) => serialize_struct(params, fields, &cont.attrs),
        Data::Struct(Style::Tuple, fields) => serialize_tuple_struct(params, fields, &cont.attrs),
        Data::Struct(Style::Newtype, fields) => {
            serialize_newtype_struct(params, fields, &cont.attrs)
        }
        Data::Struct(Style::Unit, _) => serialize_unit_struct(&cont.attrs),
    }
}

fn serialize_into(params: &Parameters, type_into: &syn::Type) -> Fragment {
    let self_var = &params.self_var;
    quote_block! {
        _serde::Serialize::serialize(
            &_serde::export::Into::<#type_into>::into(_serde::export::Clone::clone(#self_var)),
            __serializer)
    }
}

fn serialize_unit_struct(_cattrs: &attr::Container) -> (Fragment, Fragment) {
    (
        quote_expr! {
            Ok(__buf__)
        },
        quote_expr! {
            Ok(0)
        },
    )
}

fn serialize_newtype_struct(
    params: &Parameters,
    fields: &[Field],
    _cattrs: &attr::Container,
) -> (Fragment, Fragment) {
    let (flag, flag_measure) = encode_tuple_flag(params, fields, false, _cattrs.optimize_option);
    let mut field_expr = get_member(
        params,
        &fields[0],
        &Member::Unnamed(Index {
            index: 0,
            span: Span::call_site(),
        }),
    );

    (
        quote_expr! {
            #flag
            let __buf__ = #field_expr.raw_encode(__buf__, __purpose__)?;
            Ok(__buf__)
        },
        quote_expr! {
            Ok(#flag_measure + #field_expr.raw_measure(__purpose__)?)
        },
    )
}

fn serialize_tuple_struct(
    params: &Parameters,
    fields: &[Field],
    _cattrs: &attr::Container,
) -> (Fragment, Fragment) {
    if fields.len() == 0 {
        (
            quote_block! {
                Ok(__buf__)
            },
            quote_block! {
                Ok((0))
            },
        )
    } else {
        let (flag, serialize_stmts, flag_measure, serialize_stmts1) =
            encode_tuple_struct_visitor(fields, params, false, _cattrs.optimize_option);

        (
            quote_block! {
                #flag
                #(#serialize_stmts)*
                Ok(__buf__)
            },
            quote_block! {
                Ok(#flag_measure + #(#serialize_stmts1)+*)
            },
        )
    }
}

fn serialize_struct(
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
) -> (Fragment, Fragment) {
    assert!(fields.len() as u64 <= u64::from(u32::max_value()));

    serialize_struct_as_struct(params, fields, cattrs)
}

fn serialize_struct_tag_field(_cattrs: &attr::Container) -> TokenStream {
    quote! {}
}

fn serialize_struct_as_struct(
    params: &Parameters,
    fields: &[Field],
    _cattrs: &attr::Container,
) -> (Fragment, Fragment) {
    if fields.len() == 0 {
        (
            quote_block! {
                Ok(__buf__)
            },
            quote_block! {
                Ok((0))
            },
        )
    } else {
        let (flag, serialize_fields, flag_measure, serialize_fields1) =
            encode_struct_visitor(fields, params, false, _cattrs.optimize_option);

        (
            quote_block! {
                #flag
                #(#serialize_fields)*
                Ok(__buf__)
            },
            quote_block! {
                Ok(#flag_measure + #(#serialize_fields1)+*)
            },
        )
    }
}

fn encode_enum(
    params: &Parameters,
    variants: &[Variant],
    optimize_option: bool,
) -> (Fragment, Fragment) {
    assert!(variants.len() as u64 <= u64::from(u32::max_value()));

    let self_var = &params.self_var;

    if variants.len() == 0 {
        (quote_block!(Ok(__buf__)), quote_block!(Ok(0)))
    } else {
        let arms: Vec<_> = variants
            .iter()
            .enumerate()
            .map(|(variant_index, variant)| {
                encode_enum_variant(params, variant, variant_index, optimize_option)
            })
            .collect();

        let arms1: Vec<_> = arms
            .iter()
            .enumerate()
            .map(|(_, (v1, _))| v1.clone())
            .collect();
        let arms2: Vec<_> = arms
            .iter()
            .enumerate()
            .map(|(_, (_, v2))| v2.clone())
            .collect();
        (
            quote_expr! {
                match #self_var {
                    #(#arms1)*
                }
            },
            quote_expr! {
                match #self_var {
                    #(#arms2)*
                }
            },
        )
    }
}

fn encode_enum_variant(
    params: &Parameters,
    variant: &Variant,
    variant_index: usize,
    optimize_option: bool,
) -> (TokenStream, TokenStream) {
    let this = &params.this;
    let variant_ident = &variant.ident;

    // variant wasn't skipped
    let case = match variant.style {
        Style::Unit => {
            quote! {
                #this::#variant_ident
            }
        }
        Style::Newtype => {
            quote! {
                #this::#variant_ident(ref __field0)
            }
        }
        Style::Tuple => {
            let field_names = (0..variant.fields.len())
                .map(|i| Ident::new(&format!("__field{}", i), Span::call_site()));
            quote! {
                #this::#variant_ident(#(ref #field_names),*)
            }
        }
        Style::Struct => {
            let members = variant.fields.iter().map(|f| &f.member);
            quote! {
                #this::#variant_ident { #(ref #members),* }
            }
        }
    };

    let (body, measure_body) =
        encode_enum_inner_variant(params, variant, variant_index, optimize_option);
    let body = Stmts(body);
    let measure_body = Stmts(measure_body);
    (
        quote! {
            #case => #body
        },
        quote! {
            #case => #measure_body
        },
    )
}

fn encode_enum_inner_variant(
    params: &Parameters,
    variant: &Variant,
    variant_index: usize,
    optimize_option: bool,
) -> (Fragment, Fragment) {
    let variant_name = variant.ident.to_string();
    let type_name = String::from("");
    match variant.style {
        Style::Unit => (
            quote_block!({
                let __buf__ = cyfs_base::USize(#variant_index).raw_encode(__buf__, __purpose__)?;
                Ok(__buf__)
            }),
            quote_block!({
                Ok(cyfs_base::USize(#variant_index).raw_measure(__purpose__)?)
            }),
        ),
        Style::Newtype => {
            let _field = &variant.fields[0];
            let field_expr = quote!(__field0);
            let (flag, flag_measure) =
                encode_tuple_flag(params, &variant.fields, true, optimize_option);

            (
                if optimize_option && _field.is_option() {
                    quote_block! {{
                        let __buf__ = cyfs_base::USize(#variant_index).raw_encode(__buf__, __purpose__)?;
                        #flag
                        let __buf__ = if #field_expr.is_some() {
                            #field_expr.as_ref().unwrap().raw_encode(__buf__, __purpose__)?
                        } else {
                            __buf__
                        };
                        Ok(__buf__)
                    }}
                } else if _field.is_vec_u8() {
                    let item = quote_block! {{
                        let __buf__ = cyfs_base::USize(#variant_index).raw_encode(__buf__, __purpose__)?;
                        #flag
                        let __buf__ = {
                            let __buf__ = cyfs_base::USize(#field_expr.len()).raw_encode(__buf__, __purpose__)?;
                            if #field_expr.len() == 0 {
                                __buf__
                            } else {
                                unsafe {
                                    std::ptr::copy::<u8>(#field_expr.as_ptr() as *mut u8, __buf__.as_mut_ptr(), #field_expr.len());
                                }
                                &mut __buf__[#field_expr.len()..]
                            }
                        };
                        Ok(__buf__)
                    }};
                    // println!("{}", item.as_ref().to_string());
                    item
                } else {
                    quote_block! {{
                        let __buf__ = cyfs_base::USize(#variant_index).raw_encode(__buf__, __purpose__)?;
                        #flag
                        let __buf__ = #field_expr.raw_encode(__buf__, __purpose__)?;
                        Ok(__buf__)
                    }}
                },
                if optimize_option && _field.is_option() {
                    quote_block! {{
                        Ok(cyfs_base::USize(#variant_index).raw_measure(__purpose__)? + #flag_measure + if #field_expr.is_some() {#field_expr.as_ref().unwrap().raw_measure(__purpose__)?} else {0})
                    }}
                } else if _field.is_vec_u8() {
                    quote_block!({
                        Ok(cyfs_base::USize(#variant_index).raw_measure(__purpose__)? + #flag_measure + cyfs_base::USize(#field_expr.len()).raw_measure(__purpose__)? + #field_expr.len())
                    })
                } else {
                    quote_block!({
                    Ok(cyfs_base::USize(#variant_index).raw_measure(__purpose__)? + #flag_measure + #field_expr.raw_measure(__purpose__)?)
                    })
                },
            )
        }
        Style::Tuple => encode_enum_tuple_variant(
            TupleVariant::ExternallyTagged {
                type_name,
                variant_index,
                variant_name,
            },
            params,
            &variant.fields,
            optimize_option,
        ),
        Style::Struct => encode_enum_struct_variant(
            StructVariant::ExternallyTagged {
                variant_index,
                variant_name,
            },
            params,
            &variant.fields,
            optimize_option,
        ),
    }
}

enum TupleVariant {
    ExternallyTagged {
        type_name: String,
        variant_index: usize,
        variant_name: String,
    },
    Untagged,
}

fn encode_enum_tuple_variant(
    context: TupleVariant,
    params: &Parameters,
    fields: &[Field],
    optimize_option: bool,
) -> (Fragment, Fragment) {
    let (flag, encode_stmts, flag_measure, measure_encode_stmts) =
        encode_tuple_struct_visitor(fields, params, true, optimize_option);

    match context {
        TupleVariant::ExternallyTagged {
            type_name: _,
            variant_index,
            variant_name: _,
        } => {
            if encode_stmts.len() == 0 {
                (
                    quote_block!({
                        let __buf__ = cyfs_base::USize(#variant_index).raw_encode(__buf__, __purpose__)?;
                        Ok(__buf__)
                    }),
                    quote_block!({
                        Ok(cyfs_base::USize(#variant_index).raw_measure(__purpose__)?)
                    }),
                )
            } else {
                (
                    quote_block!({
                        let __buf__ = cyfs_base::USize(#variant_index).raw_encode(__buf__, __purpose__)?;
                        #flag
                        #(#encode_stmts)*
                        Ok(__buf__)
                    }),
                    quote_block!({
                        Ok(cyfs_base::USize(#variant_index).raw_measure(__purpose__)? + #flag_measure + #(#measure_encode_stmts)+*)
                    }),
                )
            }
        }
        TupleVariant::Untagged => (
            quote_block!({
                #(#encode_stmts)*
                Ok(__buf__)
            }),
            quote_block!({
                Ok(#(#measure_encode_stmts)*)
            }),
        ),
    }
}

enum StructVariant<'a> {
    ExternallyTagged {
        variant_index: usize,
        variant_name: String,
    },
    InternallyTagged {
        tag: &'a str,
        variant_name: String,
    },
}

fn encode_enum_struct_variant<'a>(
    context: StructVariant<'a>,
    params: &Parameters,
    fields: &[Field],
    optimize_option: bool,
) -> (Fragment, Fragment) {
    let (flag, encode_fields, flag_measure, measure_fields) =
        encode_struct_visitor(fields, params, true, optimize_option);

    match context {
        StructVariant::ExternallyTagged {
            variant_index,
            variant_name: _,
        } => {
            if encode_fields.len() == 0 {
                (
                    quote_block!({
                        let __buf__ = cyfs_base::USize(#variant_index).raw_encode(__buf__, __purpose__)?;
                        Ok(__buf__)
                    }),
                    quote_block!({
                        Ok(cyfs_base::USize(#variant_index).raw_measure(__purpose__)?)
                    }),
                )
            } else {
                (
                    quote_block!({
                        let __buf__ = cyfs_base::USize(#variant_index).raw_encode(__buf__, __purpose__)?;
                        #flag
                        #(#encode_fields)*
                        Ok(__buf__)
                    }),
                    quote_block!({
                        Ok(cyfs_base::USize(#variant_index).raw_measure(__purpose__)? + #flag_measure + #(#measure_fields)+*)
                    }),
                )
            }
        }
        StructVariant::InternallyTagged {
            tag: _,
            variant_name: _,
        } => (
            quote_block!({
                Ok(#(#encode_fields)*)
            }),
            quote_block!({
                Ok(#(#measure_fields)+*)
            }),
        ),
    }
}

fn encode_tuple_flag(
    params: &Parameters,
    fields: &[Field],
    is_enum: bool,
    optimize_option: bool,
) -> (TokenStream, TokenStream) {
    let mut option_index = 0;
    let flag_stmts: Vec<_> = if is_enum {
        fields
            .iter()
            .enumerate()
            .filter(|(_, field)| optimize_option && field.is_option())
            .map(|(i, field)| {
                let name = {
                    let id = Ident::new(&format!("__field{}", i), Span::call_site());
                    quote!(#id)
                };
                let item = quote! {
                    if #name.is_some() {
                        flag |= 1 << #option_index;
                    }
                };
                option_index += 1;
                item
            })
            .collect()
    } else {
        fields
            .iter()
            .enumerate()
            .filter(|(_, field)| optimize_option && field.is_option())
            .map(|(i, field)| {
                let name = {
                    get_member(
                        params,
                        field,
                        &Member::Unnamed(Index {
                            index: i as u32,
                            span: Span::call_site(),
                        }),
                    )
                };
                let item = quote! {
                    if #name.is_some() {
                        flag |= 1 << #option_index;
                    }
                };
                option_index += 1;
                item
            })
            .collect()
    };

    let option_count = flag_stmts.len();
    if option_count == 0 {
        return (quote! {}, quote! {0});
    }
    let (flag, flag_measure) = if option_count <= 8 {
        (
            quote! {let mut flag = 0u8;},
            quote! {u8::raw_bytes().unwrap()},
        )
    } else if option_count <= 16 {
        (
            quote! {let mut flag = 0u16;},
            quote! {u16::raw_bytes().unwrap()},
        )
    } else if option_count <= 32 {
        (
            quote! {let mut flag = 0u32;},
            quote! {u32::raw_bytes().unwrap()},
        )
    } else if option_count <= 64 {
        (
            quote! {let mut flag = 0u64;},
            quote! {u64::raw_bytes().unwrap()},
        )
    } else if option_count <= 128 {
        (
            quote! {let mut flag = 0u128;},
            quote! {u128::raw_bytes().unwrap()},
        )
    } else if option_count <= 256 {
        (
            quote! {let mut flag = 0u256;},
            quote! {u256::raw_bytes().unwrap()},
        )
    } else {
        (quote! {}, quote! {0})
    };

    (
        quote! {
            #flag
            #(#flag_stmts)*
            let __buf__ = flag.raw_encode(__buf__, __purpose__)?;
        },
        quote! {
            #flag_measure
        },
    )
}

fn encode_tuple_struct_visitor(
    fields: &[Field],
    params: &Parameters,
    is_enum: bool,
    optimize_option: bool,
) -> (TokenStream, Vec<TokenStream>, TokenStream, Vec<TokenStream>) {
    let (flag, flag_measure) = encode_tuple_flag(params, fields, is_enum, optimize_option);
    (flag, fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let field_expr = if is_enum {
                let id = Ident::new(&format!("__field{}", i), Span::call_site());
                quote!(#id)
            } else {
                get_member(
                    params,
                    field,
                    &Member::Unnamed(Index {
                        index: i as u32,
                        span: Span::call_site(),
                    }),
                )
            };
            if optimize_option && field.is_option() {
                quote! {
                    let __buf__ = if #field_expr.is_some() {
                        #field_expr.unwrap().raw_encode(__buf__, __purpose__)?
                    } else {
                        __buf__
                    };
                }
            } else if field.is_vec_u8() {
                let item = quote! {
                    let __buf__ = {
                        let __buf__ = cyfs_base::USize(#field_expr.len()).raw_encode(__buf__, __purpose__)?;
                        if #field_expr.len() == 0 {
                            __buf__
                        } else {
                            unsafe {
                                std::ptr::copy::<u8>(#field_expr.as_ptr() as *mut u8, __buf__.as_mut_ptr(), #field_expr.len());
                            }
                            &mut __buf__[#field_expr.len()..]
                        }
                    };
                };
                // println!("{}", item.to_string());
                item
            } else {
                quote! {
                    let __buf__ = #field_expr.raw_encode(__buf__, __purpose__)?;
                }
            }
        })
        .collect(),
     flag_measure,
     fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let field_expr = if is_enum {
                let id = Ident::new(&format!("__field{}", i), Span::call_site());
                quote!(#id)
            } else {
                get_member(
                    params,
                    field,
                    &Member::Unnamed(Index {
                        index: i as u32,
                        span: Span::call_site(),
                    }),
                )
            };

            let ser = if optimize_option && field.is_option() {quote! {
                if #field_expr.is_some() {
                    #field_expr.unwrap().raw_measure(__purpose__)?
                } else {
                    0
                }
            }} else if field.is_vec_u8() {
                quote!{cyfs_base::USize(#field_expr.len()).raw_measure(__purpose__)? + #field_expr.len()}
            } else {
                quote!{#field_expr.raw_measure(__purpose__)?}
            };
            ser
        })
        .collect())
}

fn encode_struct_flag(
    fields: &[Field],
    is_enum: bool,
    optimize_option: bool,
) -> (TokenStream, TokenStream) {
    let mut option_index = 0;
    let flag_stmts: Vec<_> = if is_enum {
        fields
            .iter()
            .enumerate()
            .filter(|(_, field)| optimize_option && field.is_option())
            .map(|(i, field)| {
                let name = &field.member;
                let item = quote! {
                    if #name.is_some() {
                        flag |= 1 << #option_index;
                    }
                };
                option_index += 1;
                item
            })
            .collect()
    } else {
        fields
            .iter()
            .enumerate()
            .filter(|(_, field)| optimize_option && field.is_option())
            .map(|(i, field)| {
                let name = &field.member;
                let item = quote! {
                    if self.#name.is_some() {
                        flag |= 1 << #option_index;
                    }
                };
                option_index += 1;
                item
            })
            .collect()
    };

    let option_count = flag_stmts.len();
    if option_count == 0 {
        return (quote! {}, quote! {0});
    }
    let (flag, flag_measure) = if option_count <= 8 {
        (
            quote! {let mut flag = 0u8;},
            quote! {u8::raw_bytes().unwrap()},
        )
    } else if option_count <= 16 {
        (
            quote! {let mut flag = 0u16;},
            quote! {u16::raw_bytes().unwrap()},
        )
    } else if option_count <= 32 {
        (
            quote! {let mut flag = 0u32;},
            quote! {u32::raw_bytes().unwrap()},
        )
    } else if option_count <= 64 {
        (
            quote! {let mut flag = 0u64;},
            quote! {u64::raw_bytes().unwrap()},
        )
    } else if option_count <= 128 {
        (
            quote! {let mut flag = 0u128;},
            quote! {u128::raw_bytes().unwrap()},
        )
    } else if option_count <= 256 {
        (
            quote! {let mut flag = 0u256;},
            quote! {u256::raw_bytes().unwrap()},
        )
    } else {
        (quote! {}, quote! {0})
    };

    (
        quote! {
            #flag
            #(#flag_stmts)*
            let __buf__ = flag.raw_encode(__buf__, __purpose__)?;
        },
        quote! {
            #flag_measure
        },
    )
}

fn encode_struct_visitor(
    fields: &[Field],
    _params: &Parameters,
    _is_enum: bool,
    optimize_option: bool,
) -> (TokenStream, Vec<TokenStream>, TokenStream, Vec<TokenStream>) {
    let (flag, flag_measure) = encode_struct_flag(fields, _is_enum, optimize_option);
    if _is_enum {
        (flag, fields
             .iter()
            .filter(|field| !field.attrs.skip_serializing())
             .map(|field| {
                 let ty = field.ty;
                 let member = &field.member;
                 if optimize_option && field.is_option() {
                    quote! {
                        let __buf__ = if #member.is_some() {
                            #member.unwrap().raw_encode(__buf__, __purpose__)?
                        } else {
                            __buf__
                        }
                    }
                 } else if field.is_vec_u8() {
                     let item = quote! {
                        let __buf__ = {
                            let __buf__ = cyfs_base::USize(#member.len()).raw_encode(__buf__, __purpose__)?;
                            if #member.len() == 0 {
                                __buf__
                            } else {
                                unsafe {
                                    std::ptr::copy::<u8>(#member.as_ptr() as *mut u8, __buf__.as_mut_ptr(), #member.len());
                                }
                                &mut __buf__[#member.len()..]
                            }
                        };
                     };
                     // println!("{}", item.to_string());
                     item
                 } else {
                     quote! {
                        let __buf__ = #member.raw_encode(__buf__, __purpose__)?;
                    }
                 }
             })
             .collect(),

         flag_measure, fields
             .iter()
             .filter(|field| !field.attrs.skip_serializing())
             .map(|field| {
                 let member = &field.member;

                 if optimize_option && field.is_option() {
                     quote! {
                        {if #member.is_some() {
                            #member.as_ref().unwrap().raw_measure(__purpose__)?
                        } else {
                            0
                        }}
                    }
                 } else if field.is_vec_u8() {
                     quote!{cyfs_base::USize(#member.len()).raw_measure(__purpose__)? + #member.len()}
                 } else {
                     quote! {
                        #member.raw_measure(__purpose__)?
                    }
                 }
             })
             .collect())
    } else {
        (flag, fields
             .iter()
            .filter(|field| !field.attrs.skip_serializing())
             .map(|field| {
                 let member = &field.member;
                 if optimize_option && field.is_option() {
                     quote! {
                        let __buf__ = if self.#member.is_some() {
                            self.#member.as_ref().unwrap().raw_encode(__buf__, __purpose__)?
                        } else {
                            __buf__
                        };
                    }
                 } else if field.is_vec_u8() {
                     let item = quote! {
                        let __buf__ = {
                            let __buf__ = cyfs_base::USize(self.#member.len()).raw_encode(__buf__, __purpose__)?;
                            if self.#member.len() == 0 {
                                __buf__
                            } else {
                                unsafe {
                                    std::ptr::copy::<u8>(self.#member.as_ptr() as *mut u8, __buf__.as_mut_ptr(), self.#member.len());
                                }
                                &mut __buf__[self.#member.len()..]
                            }
                        };
                    };
                     // println!("{}", item.to_string());
                     item
                 } else {
                     quote! {
                        let __buf__ = self.#member.raw_encode(__buf__, __purpose__)?;
                    }
                 }
             })
             .collect(),

         flag_measure, fields
             .iter()
             .filter(|field| !field.attrs.skip_serializing())
             .map(|field| {
                 let member = &field.member;
                 if optimize_option && field.is_option() {
                     quote! {
                        {if self.#member.is_some() {
                            self.#member.as_ref().unwrap().raw_measure(__purpose__)?
                        } else {
                            0
                        }}
                    }
                 } else if field.is_vec_u8() {
                     quote!{cyfs_base::USize(self.#member.len()).raw_measure(__purpose__)? + self.#member.len()}
                 } else {
                     quote! {
                        self.#member.raw_measure(__purpose__)?
                     }
                 }
             })
             .collect())
    }
}

// Serialization of an empty struct results in code like:
//
//     let mut __serde_state = try_tmp!(serializer.serialize_struct("S", 0));
//     _serde::ser::SerializeStruct::end(__serde_state)
//
// where we want to omit the `mut` to avoid a warning.
fn mut_if(is_mut: bool) -> Option<TokenStream> {
    if is_mut {
        Some(quote!(mut))
    } else {
        None
    }
}

fn get_member(params: &Parameters, _field: &Field, member: &Member) -> TokenStream {
    let self_var = &params.self_var;
    quote!(#self_var.#member)
}
