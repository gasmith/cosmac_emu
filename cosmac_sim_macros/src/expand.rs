use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Result};

use crate::ast::{Enum, Input};

pub fn derive(node: &DeriveInput) -> Result<TokenStream> {
    let input = Input::from_syn(node)?;
    Ok(match input {
        Input::Enum(input) => impl_enum(input),
    })
}

fn impl_enum(input: Enum) -> TokenStream {
    let ty = &input.ident;

    let mut decode_packed_arms = vec![];
    let mut decode_plain_arms = vec![];
    for v in &input.variants {
        let ident = &v.ident;
        let opcode = v.schema.opcode;
        if v.schema.packed {
            decode_packed_arms.push(quote! {
                #opcode => Some(Self::#ident(opcode_lo)),
            });
        } else if v.schema.size == 1 {
            decode_plain_arms.push(quote! {
                #opcode => Some(Self::#ident),
            });
        } else if v.schema.size == 2 {
            decode_plain_arms.push(quote! {
                #opcode => {
                    match arg1 {
                        Some(arg1) => Some(Self::#ident(*arg1)),
                        _=> None,
                    }
                }
            });
        } else if v.schema.size == 3 {
            decode_plain_arms.push(quote! {
                #opcode => {
                    match (arg1, arg2) {
                        (Some(arg1), Some(arg2)) => Some(Self::#ident(*arg1, *arg2)),
                        _ => None,
                    }
                }
            });
        }
    }
    let decode = quote! {
        fn decode(bin: &[u8]) -> Option<Self> {
            if let Some(opcode) = bin.get(0) {
                let opcode_lo = opcode & 0x0f;
                let arg1 = bin.get(1);
                let arg2 = bin.get(2);
                match opcode {
                    #(#decode_plain_arms)*
                    _ => match opcode & 0xf0 {
                        #(#decode_packed_arms)*
                        _ => None,
                    },
                }
            } else {
                None
            }
        }
    };

    let disasm_arms = input.variants.iter().map(|v| {
        let ident = &v.ident;
        let mnemonic = ident.to_string().to_lowercase();
        match v.schema.arity() {
            0 => quote! {
                #ty::#ident => format!("{:<4}", #mnemonic),
            },
            1 => quote! {
                #ty::#ident(n) => format!("{:<4} {}", #mnemonic, n),
            },
            2 => quote! {
                #ty::#ident(hh, ll) => format!("{:<4} {} {}", #mnemonic, hh, ll),
            },
            _ => unreachable!(),
        }
    });
    let disasm = quote! {
        fn disasm(&self) -> String {
            match self {
                #(#disasm_arms)*
            }
        }
    };

    let encode_arms = input.variants.iter().map(|v| {
        let ident = &v.ident;
        let opcode = v.schema.opcode;
        if v.schema.packed {
            quote! {
                #ty::#ident(n) => vec![#opcode | n],
            }
        } else if v.schema.size == 1 {
            quote! {
                #ty::#ident => vec![#opcode],
            }
        } else if v.schema.size == 2 {
            quote! {
                #ty::#ident(nn) => vec![#opcode, *nn],
            }
        } else {
            assert_eq!(v.schema.size, 3);
            quote! {
                #ty::#ident(hh, ll) => vec![#opcode, *hh, *ll],
            }
        }
    });
    let encode = quote! {
        fn encode(&self) -> Vec<u8> {
            match self {
                #(#encode_arms)*
            }
        }
    };

    let size_arms = input.variants.iter().map(|v| {
        let ident = &v.ident;
        let size = v.schema.size;
        match v.schema.arity() {
            0 => quote! {
                #ty::#ident => #size,
            },
            1 => quote! {
                #ty::#ident(_) => #size,
            },
            2 => quote! {
                #ty::#ident(_, _) => #size,
            },
            _ => unreachable!(),
        }
    });
    let size = quote! {
        fn size(&self) -> u8{
            match self {
                #(#size_arms)*
            }
        }
    };

    quote! {
        impl InstrSchema for #ty {
            #decode
            #disasm
            #encode
            #size
        }
    }
    .into()
}
