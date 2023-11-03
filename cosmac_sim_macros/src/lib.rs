extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod ast;
mod expand;
mod schema;

#[proc_macro_derive(InstrSchema, attributes(schema))]
pub fn derive_instr_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand::derive(&input)
        .unwrap_or_else(|err| err.to_compile_error().into())
        .into()
}
