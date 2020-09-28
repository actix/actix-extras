use serde::{Deserialize, Deserializer};
use serde_json::from_str;
use std::collections::{HashMap, HashSet};
use std::str::{FromStr, ParseBoolError};
use std::sync::Mutex;

lazy_static! {
    static ref PRIMITIVE_TYPES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("*", "Variant");
        m.insert("binary", "Bytes");
        m.insert("string", "ByteString");
        m.insert("ubyte", "u8");
        m.insert("ushort", "u16");
        m.insert("uint", "u32");
        m.insert("ulong", "u64");
        m.insert("boolean", "bool");
        m
    };
    static ref STRING_TYPES: HashSet<&'static str> = ["string", "symbol"].iter().cloned().collect();
    static ref REF_TYPES: Mutex<HashSet<String>> = Mutex::new(
        [
            "Bytes",
            "ByteString",
            "Symbol",
            "Fields",
            "Map",
            "MessageId",
            "Address",
            "NodeProperties",
            "Outcome",
            "DeliveryState",
            "FilterSet",
            "DeliveryTag",
            "Symbols",
            "IetfLanguageTags",
            "ErrorCondition",
            "DistributionMode"
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    );
    static ref ENUM_TYPES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

pub fn parse(spec: &str) -> Definitions {
    let types = from_str::<Vec<_Type>>(spec).expect("Failed to parse AMQP spec.");
    {
        let mut ref_map = REF_TYPES.lock().unwrap();
        let mut enum_map = ENUM_TYPES.lock().unwrap();
        for t in types.iter() {
            match *t {
                _Type::Described(ref l) if l.source == "list" => {
                    ref_map.insert(camel_case(&*l.name));
                }
                _Type::Choice(ref e) => {
                    enum_map.insert(camel_case(&*e.name));
                }
                _ => {}
            }
        }
    }

    Definitions::from(types)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum _Type {
    Choice(_Enum),
    Described(_Described),
    Alias(_Alias),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct _Descriptor {
    name: String,
    code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct _Enum {
    name: String,
    source: String,
    provides: Option<String>,
    choice: Vec<EnumItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct _Described {
    name: String,
    class: String,
    source: String,
    provides: Option<String>,
    descriptor: _Descriptor,
    #[serde(default)]
    field: Vec<_Field>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct _Field {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    #[serde(deserialize_with = "string_as_bool")]
    mandatory: bool,
    default: Option<String>,
    #[serde(default)]
    #[serde(deserialize_with = "string_as_bool")]
    multiple: bool,
    requires: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct _Alias {
    name: String,
    source: String,
    provides: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alias {
    name: String,
    source: String,
    provides: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumItem {
    name: String,
    value: String,
    #[serde(default)]
    value_len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definitions {
    aliases: Vec<Alias>,
    enums: Vec<Enum>,
    lists: Vec<Described>,
    described_restricted: Vec<Described>,
    provides: Vec<ProvidesEnum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProvidesEnum {
    name: String,
    described: bool,
    options: Vec<ProvidesItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProvidesItem {
    ty: String,
    descriptor: Descriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enum {
    name: String,
    ty: String,
    provides: Vec<String>,
    items: Vec<EnumItem>,
    is_symbol: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Described {
    name: String,
    ty: String,
    provides: Vec<String>,
    descriptor: Descriptor,
    fields: Vec<Field>,
    transfer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Descriptor {
    name: String,
    domain: u32,
    code: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    name: String,
    ty: String,
    is_str: bool,
    is_ref: bool,
    optional: bool,
    default: String,
    multiple: bool,
}

impl Definitions {
    fn from(types: Vec<_Type>) -> Definitions {
        let mut aliases = vec![];
        let mut enums = vec![];
        let mut lists = vec![];
        let mut described_restricted = vec![];
        let mut provide_map: HashMap<String, Vec<ProvidesItem>> = HashMap::new();
        for t in types.into_iter() {
            match t {
                _Type::Alias(ref a) if a.source != "map" => {
                    let al = Alias::from(a.clone());
                    Definitions::register_provides(&mut provide_map, &al.name, None, &al.provides);
                    aliases.push(al);
                }
                _Type::Choice(ref e) => {
                    let en = Enum::from(e.clone());
                    Definitions::register_provides(&mut provide_map, &en.name, None, &en.provides);
                    enums.push(en);
                }
                _Type::Described(ref d) if d.source == "list" && d.class != "restricted" => {
                    let ls = Described::list(d.clone());
                    Definitions::register_provides(
                        &mut provide_map,
                        &ls.name,
                        Some(ls.descriptor.clone()),
                        &ls.provides,
                    );
                    lists.push(ls);
                }
                _Type::Described(ref d) if d.class == "restricted" => {
                    let ls = Described::alias(d.clone());
                    Definitions::register_provides(
                        &mut provide_map,
                        &ls.name,
                        Some(ls.descriptor.clone()),
                        &ls.provides,
                    );
                    described_restricted.push(ls);
                }
                _ => {}
            }
        }

        let provides = provide_map
            .into_iter()
            .filter_map(|(k, v)| {
                if v.len() == 1 {
                    None
                } else {
                    Some(ProvidesEnum {
                        described: if k == "Frame" {
                            false
                        } else {
                            v.iter().any(|v| v.descriptor.code != 0)
                        },
                        name: k,
                        options: v,
                    })
                }
            })
            .collect();

        Definitions {
            aliases,
            enums,
            lists,
            described_restricted,
            provides,
        }
    }

    fn register_provides(
        map: &mut HashMap<String, Vec<ProvidesItem>>,
        name: &str,
        descriptor: Option<Descriptor>,
        provides: &Vec<String>,
    ) {
        for p in provides.iter() {
            map.entry(p.clone())
                .or_insert_with(|| vec![])
                .push(ProvidesItem {
                    ty: name.to_string(),
                    descriptor: descriptor.clone().unwrap_or_else(|| Descriptor {
                        name: String::new(),
                        domain: 0,
                        code: 0,
                    }),
                });
        }
    }
}

impl Alias {
    fn from(a: _Alias) -> Alias {
        Alias {
            name: camel_case(&*a.name),
            source: get_type_name(&*a.source, None),
            provides: parse_provides(a.provides),
        }
    }
}

impl Enum {
    fn from(e: _Enum) -> Enum {
        let ty = get_type_name(&*e.source, None);
        let is_symbol = ty == "Symbol";
        Enum {
            name: camel_case(&*e.name),
            ty: ty.clone(),
            provides: parse_provides(e.provides),
            is_symbol,
            items: e
                .choice
                .into_iter()
                .map(|c| EnumItem {
                    name: camel_case(&*c.name),
                    value_len: c.value.len(),
                    value: c.value,
                })
                .collect(),
        }
    }
}

impl Described {
    fn list(d: _Described) -> Described {
        let transfer = d.name == "transfer";
        Described {
            name: camel_case(&d.name),
            ty: String::new(),
            provides: parse_provides(d.provides),
            descriptor: Descriptor::from(d.descriptor),
            fields: d.field.into_iter().map(|f| Field::from(f)).collect(),
            transfer,
        }
    }
    fn alias(d: _Described) -> Described {
        Described {
            name: camel_case(&d.name),
            ty: get_type_name(&d.source, None),
            provides: parse_provides(d.provides),
            descriptor: Descriptor::from(d.descriptor),
            fields: d.field.into_iter().map(|f| Field::from(f)).collect(),
            transfer: false,
        }
    }
}

impl Descriptor {
    fn from(d: _Descriptor) -> Descriptor {
        let code_parts: Vec<u32> = d
            .code
            .split(":")
            .map(|p| {
                assert!(p.starts_with("0x"));
                u32::from_str_radix(&p[2..], 16).expect("malformed descriptor code")
            })
            .collect();
        Descriptor {
            name: d.name,
            domain: code_parts[0],
            code: code_parts[1],
        }
    }
}
impl Field {
    fn from(field: _Field) -> Field {
        let mut ty = get_type_name(&*field.ty, field.requires);
        if field.multiple {
            ty.push('s');
        }
        let is_str = STRING_TYPES.contains(&*ty) && !field.multiple;
        let is_ref = REF_TYPES.lock().unwrap().contains(&ty);
        let default = Field::format_default(field.default, &ty);
        Field {
            name: snake_case(&*field.name),
            ty: ty,
            is_ref,
            is_str,
            optional: !field.mandatory && default.len() == 0,
            multiple: field.multiple,
            default,
        }
    }

    fn format_default(default: Option<String>, ty: &str) -> String {
        match default {
            None => String::new(),
            Some(def) => {
                if ENUM_TYPES.lock().unwrap().contains(ty) {
                    format!("{}::{}", ty, camel_case(&*def))
                } else {
                    def
                }
            }
        }
    }
}

fn get_type_name(ty: &str, req: Option<String>) -> String {
    match req {
        Some(t) => camel_case(&*t),
        None => match PRIMITIVE_TYPES.get(ty) {
            Some(p) => p.to_string(),
            None => camel_case(&*ty),
        },
    }
}

fn parse_provides(p: Option<String>) -> Vec<String> {
    p.map(|v| {
        v.split_terminator(",")
            .filter_map(|s| {
                let s = s.trim();
                if s == "" {
                    None
                } else {
                    Some(camel_case(&s))
                }
            })
            .collect()
    })
    .unwrap_or(vec![])
}

fn string_as_bool<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr<Err = ParseBoolError>,
    D: Deserializer<'de>,
{
    Ok(String::deserialize(deserializer)?
        .parse::<T>()
        .expect("Error parsing bool from string"))
}

pub fn camel_case(name: &str) -> String {
    let mut new_word = true;
    name.chars().fold("".to_string(), |mut result, ch| {
        if ch == '-' || ch == '_' || ch == ' ' {
            new_word = true;
            result
        } else {
            result.push(if new_word {
                ch.to_ascii_uppercase()
            } else {
                ch
            });
            new_word = false;
            result
        }
    })
}

pub fn snake_case(name: &str) -> String {
    match name {
        "type" => "type_".to_string(),
        "return" => "return_".to_string(),
        name => {
            let mut new_word = false;
            let mut last_was_upper = false;
            name.chars().fold("".to_string(), |mut result, ch| {
                if ch == '-' || ch == '_' || ch == ' ' {
                    new_word = true;
                    result
                } else {
                    let uppercase = ch.is_uppercase();
                    if new_word || (!last_was_upper && !result.is_empty() && uppercase) {
                        result.push('_');
                        new_word = false;
                    }
                    last_was_upper = uppercase;
                    result.push(if uppercase {
                        ch.to_ascii_lowercase()
                    } else {
                        ch
                    });
                    result
                }
            })
        }
    }
}
