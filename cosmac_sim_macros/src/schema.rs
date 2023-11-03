use regex::Regex;
use std::sync::OnceLock;
use syn::parse::ParseStream;
use syn::{Attribute, Error, LitStr, Result};

#[derive(Debug, PartialEq)]
pub struct Schema {
    pub opcode: u8,
    pub size: u8,
    pub packed: bool,
}

impl Schema {
    pub fn parse_from_attribute(attr: &Attribute) -> Result<Schema> {
        attr.parse_args_with(|input: ParseStream| {
            let lit: LitStr = input.parse()?;
            Self::parse(&lit.value()).ok_or_else(|| Error::new_spanned(attr, "invalid schema"))
        })
    }

    fn parse(input: &str) -> Option<Schema> {
        struct Patterns {
            packed: Regex,
            plain: Regex,
        }
        static PATTERNS: OnceLock<Patterns> = OnceLock::new();
        let pat = PATTERNS.get_or_init(|| Patterns {
            packed: Regex::new(r"^([0-9a-f])n$").unwrap(),
            plain: Regex::new(r"^([0-9a-f]{2})(| nn| hh ll| xx xx)$").unwrap(),
        });
        if let Some(c) = pat.packed.captures(input) {
            let opcode = u8::from_str_radix(c.get(1).unwrap().as_str(), 16).unwrap() << 4;
            Some(Schema {
                opcode,
                size: 1,
                packed: true,
            })
        } else if let Some(c) = pat.plain.captures(input) {
            let opcode = u8::from_str_radix(c.get(1).unwrap().as_str(), 16).unwrap();
            let size = match c.get(2).unwrap().as_str() {
                "" => 1,
                " nn" => 2,
                " hh ll" => 3,
                _ => unreachable!(),
            };
            Some(Schema {
                opcode,
                size,
                packed: false,
            })
        } else {
            None
        }
    }

    pub fn arity(&self) -> u8 {
        if self.packed {
            1
        } else {
            self.size - 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Schema;

    #[test]
    fn test_schema_from_str() {
        assert!(Schema::parse("").is_none());
        assert!(Schema::parse("1").is_none());
        assert!(Schema::parse("n").is_none());
        assert!(Schema::parse("1m").is_none());
        assert!(Schema::parse("1n xx").is_none());
        assert!(Schema::parse("1n nn").is_none());
        assert_eq!(
            Schema::parse("0n").unwrap(),
            Schema {
                opcode: 0x00,
                size: 1,
                packed: true,
            }
        );
        assert_eq!(
            Schema::parse("f1").unwrap(),
            Schema {
                opcode: 0xf1,
                size: 1,
                packed: false,
            }
        );
        assert_eq!(
            Schema::parse("30 nn").unwrap(),
            Schema {
                opcode: 0x30,
                size: 2,
                packed: false,
            }
        );
        assert_eq!(
            Schema::parse("c3 hh ll").unwrap(),
            Schema {
                opcode: 0xc3,
                size: 3,
                packed: false,
            }
        );
    }
}
