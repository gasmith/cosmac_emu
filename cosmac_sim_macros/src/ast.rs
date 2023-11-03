use syn::{Data, DataEnum, DeriveInput, Error, Ident, Result};

use crate::schema::Schema;

pub enum Input<'a> {
    Enum(Enum<'a>),
}

pub struct Enum<'a> {
    pub original: &'a DeriveInput,
    pub ident: Ident,
    pub variants: Vec<Variant<'a>>,
}

pub struct Variant<'a> {
    pub original: &'a syn::Variant,
    pub schema: Schema,
    pub ident: Ident,
}

impl<'a> Input<'a> {
    pub fn from_syn(node: &'a DeriveInput) -> Result<Self> {
        match &node.data {
            Data::Enum(data) => Enum::from_syn(node, data).map(Input::Enum),
            _ => Err(Error::new_spanned(node, "only works for enums")),
        }
    }
}

impl<'a> Enum<'a> {
    fn from_syn(node: &'a DeriveInput, data: &'a DataEnum) -> Result<Self> {
        let variants = data
            .variants
            .iter()
            .map(Variant::from_syn)
            .collect::<Result<_>>()?;
        Ok(Enum {
            original: node,
            ident: node.ident.clone(),
            variants,
        })
    }
}

impl<'a> Variant<'a> {
    fn from_syn(node: &'a syn::Variant) -> Result<Self> {
        let mut schema: Option<Schema> = None;
        for attr in &node.attrs {
            if attr.path().is_ident("schema") {
                schema.replace(Schema::parse_from_attribute(attr)?);
            }
        }
        let schema = schema.ok_or_else(|| Error::new_spanned(node, "missing schema"))?;
        Ok(Variant {
            original: node,
            schema,
            ident: node.ident.clone(),
        })
    }
}
