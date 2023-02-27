use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Echo)]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    let DeriveInput { ident, .. } = input;

    let output = quote! {
        impl Echo for #ident {
            fn echo() -> u32 {
                echoed()
            }
        }
    };
    output.into()
}
