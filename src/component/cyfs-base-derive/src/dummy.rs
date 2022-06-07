#![allow(unused)]
use proc_macro2::{Ident, TokenStream};

use crate::try_;

pub fn wrap_in_const(
    code: TokenStream,
) -> TokenStream {
    // let try_replacement = try_::replacement();

    quote! {
            // #try_replacement
            #code
    }
}

#[allow(deprecated)]
fn unraw(ident: &Ident) -> String {
    // str::trim_start_matches was added in 1.30, trim_left_matches deprecated
    // in 1.33. We currently support rustc back to 1.15 so we need to continue
    // to use the deprecated one.
    ident.to_string().trim_left_matches("r#").to_owned()
}
