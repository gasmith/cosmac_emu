use syn::{Data, DataEnum, DeriveInput, Error, Ident, Result};

use crate::schema::Schema;

pub enum Input {
    Enum(Enum),
}

pub struct Enum {
    pub ident: Ident,
    pub variants: Vec<Variant>,
}

pub struct Variant {
    pub schema: Schema,
    pub ident: Ident,
}

impl Input {
    pub fn from_syn(node: &DeriveInput) -> Result<Self> {
        match &node.data {
            Data::Enum(data) => Enum::from_syn(node, data).map(Input::Enum),
            _ => Err(Error::new_spanned(node, "only works for enums")),
        }
    }
}

impl Enum {
    fn from_syn(node: &DeriveInput, data: &DataEnum) -> Result<Self> {
        let variants = data
            .variants
            .iter()
            .map(Variant::from_syn)
            .collect::<Result<_>>()?;
        Ok(Enum {
            ident: node.ident.clone(),
            variants,
        })
    }
}

impl Variant {
    fn from_syn(node: &syn::Variant) -> Result<Self> {
        let mut schema: Option<Schema> = None;
        for attr in &node.attrs {
            if attr.path().is_ident("schema") {
                schema.replace(Schema::parse_from_attribute(attr)?);
            }
        }
        let schema = schema.ok_or_else(|| Error::new_spanned(node, "missing schema"))?;
        Ok(Variant {
            schema,
            ident: node.ident.clone(),
        })
    }
}
