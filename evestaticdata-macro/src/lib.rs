use std::io::Write;
extern crate proc_macro;
use std::collections::HashMap;
use std::fs::File;
use quote::{ToTokens};
use syn::{Attribute, Expr, ExprLit, GenericArgument, Item, Lit, Meta, MetaNameValue, Path, PathArguments, PathSegment, TypePath};
use syn::spanned::Spanned;

#[proc_macro_attribute]
pub fn doc_sde(_args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    input
}


#[derive(Eq, PartialEq)]
enum TypeKind {
    /// Shared between multiple SDE datatypes, listed explicitly
    CommonType,
    /// Used for a single SDE datatype, listed explicitly
    ExternalType,
    /// Used for a single SDE datatype, inlined into that datatype
    InternalType,
    /// SDE datatype
    SdeFile(String),
    /// Any other item, used as default value. Items ignored
    Other
}

#[derive(Clone)]
enum Arity {
    Single,
    List,
    Map(syn::Type)
}

impl Default for Arity {
    fn default() -> Self {
        Arity::Single
    }
}

#[derive(Default, Clone)]
struct Flags {
    rename: Option<String>,
    nullable: bool,
    arity: Arity,
    type_alias: Option<String>,
    doc_override: Option<String>
}

fn parse_attrs(attrs: &mut Vec<Attribute>) -> Result<(TypeKind, Vec<String>, Flags), syn::Error> {
    let mut doc_string = Vec::new();
    let mut type_kind = TypeKind::Other;

    let mut flags = Flags::default();

    let mut retained = Vec::new();

    for attr in &*attrs {
        match &attr.meta {
            Meta::List(list) if list.path.is_ident("serde") => {
                match &*list.tokens.to_string() {
                    s if s.starts_with("rename") => {
                        if let Some(Lit::Str(str)) = list.tokens.clone().into_iter().skip(2).next().as_ref().map(proc_macro2::TokenTree::to_token_stream).and_then(|s| syn::parse2(s).ok()) {
                            flags.rename = Some(str.value());
                        } else {
                            return Err(syn::Error::new(attr.span(), "serde rename attribute not followed by a name!"))
                        }
                    },
                    "default" => flags.nullable = true,
                    s if s.starts_with("from") => {
                        if let Some(Lit::Str(str)) = list.tokens.clone().into_iter().skip(2).next().as_ref().map(proc_macro2::TokenTree::to_token_stream).and_then(|s| syn::parse2(s).ok()) {
                            flags.type_alias = Some(str.value());
                        } else {
                            return Err(syn::Error::new(attr.span(), "serde from attribute not followed by a type!"))
                        }
                    }
                    "deny_unknown_fields" => { /* no-op */ }
                    s if s.starts_with("deserialize_with") => { /* no-op */ }
                    _ => panic!("Unknown serde attr :( {:?}", list)
                }

                retained.push(attr.clone());
            }
            Meta::List(list) if list.path.is_ident("cfg_attr") && list.tokens.to_string().starts_with("feature=\"docs_export\", doc_sde") => {

                let list = if let Some(proc_macro2::TokenTree::Group(group)) = list.tokens.clone().into_iter().last() {
                    group.stream()
                } else {
                    todo!("docs_export macro without group!")
                };


                let list = list.clone().into_iter().collect::<Vec<proc_macro2::TokenTree>>();

                let attr_type = list.get(0).and_then(|tree| if let proc_macro2::TokenTree::Ident(i) = tree { Some(i) } else { None }).expect("doc_sde comment must have a type!");

                if attr_type == "common_type" {
                    type_kind = TypeKind::CommonType
                } else if attr_type == "internal_type" {
                    type_kind = TypeKind::InternalType
                } else if attr_type == "external_type" {
                    type_kind = TypeKind::ExternalType
                } else if attr_type == "sde_file" {
                    if let Some(proc_macro2::TokenTree::Punct(p)) = list.get(1) && p.as_char() == '=' {
                        if let Some(Lit::Str(str)) = list.get(2).map(proc_macro2::TokenTree::to_token_stream).and_then(|s| syn::parse2(s).ok()) {
                            type_kind = TypeKind::SdeFile(str.value());
                        } else {
                            return Err(syn::Error::new(attr.span(), "sde_file attribute not followed by a filename!"))
                        }
                    } else {
                        return Err(syn::Error::new(attr.span(), "sde_file attribute not followed by a filename!"))
                    };
                } else if attr_type == "alias_type" {
                    if let Some(proc_macro2::TokenTree::Punct(p)) = list.get(1) && p.as_char() == '=' {
                        if let Some(Lit::Str(str)) = list.get(2).map(proc_macro2::TokenTree::to_token_stream).and_then(|s| syn::parse2(s).ok()) {
                            flags.type_alias = Some(str.value());
                        } else {
                            return Err(syn::Error::new(attr.span(), "alias_type attribute not followed by a type!"))
                        }
                    } else {
                        return Err(syn::Error::new(attr.span(), "alias_type attribute not followed by a type!"))
                    };
                } else if attr_type == "rename" {
                    if let Some(proc_macro2::TokenTree::Punct(p)) = list.get(1) && p.as_char() == '=' {
                        if let Some(Lit::Str(str)) = list.get(2).map(proc_macro2::TokenTree::to_token_stream).and_then(|s| syn::parse2(s).ok()) {
                            flags.rename = Some(str.value());
                        } else {
                            return Err(syn::Error::new(attr.span(), "rename attribute not followed by a name!"))
                        }
                    } else {
                        return Err(syn::Error::new(attr.span(), "rename attribute not followed by a name!"))
                    };
                } else if attr_type == "override" {
                    if let Some(proc_macro2::TokenTree::Punct(p)) = list.get(1) && p.as_char() == '=' {
                        if let Some(Lit::Str(str)) = list.get(2).map(proc_macro2::TokenTree::to_token_stream).and_then(|s| syn::parse2(s).ok()) {
                            flags.doc_override = Some(str.value());
                        } else {
                            return Err(syn::Error::new(attr.span(), "rename attribute not followed by a name!"))
                        }
                    } else {
                        return Err(syn::Error::new(attr.span(), "rename attribute not followed by a name!"))
                    };
                } else {
                    panic!("Unknown doc_sde attribute type: `{:?}`", attr_type)
                }
            }
            Meta::NameValue(nv) if nv.path.is_ident("doc") => {
                if let MetaNameValue { value: Expr::Lit(ExprLit { lit: Lit::Str(comment), .. }), .. } = &nv {
                    doc_string.push(comment.value());
                }
            }
            _ => retained.push(attr.clone())
        };
    }

    attrs.retain(|attr| retained.contains(&attr));

    Ok((type_kind, doc_string, flags))
}

enum DocItem {
    Enum(proc_macro2::Ident, String, Vec<(proc_macro2::Ident, Option<u64>, Vec<String>, Flags)>),
    Struct(proc_macro2::Ident, String, Vec<(proc_macro2::Ident, syn::Type, Vec<String>, Flags)>),
    Override(proc_macro2::Ident, String)
}

impl DocItem {
    fn ident(&self) -> &proc_macro2::Ident {
        match self {
            DocItem::Enum(i, _, _) => i,
            DocItem::Struct(i, _, _) => i,
            DocItem::Override(i, _) => i
        }
    }
}

#[proc_macro_attribute]
pub fn doc_export(_args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match syn::parse::<syn::ItemMod>(input) {
        Ok(mut module) => {
            let mut items = Vec::<(DocItem, TypeKind, Vec<String>)>::new();

            for item in module.content.as_mut().map(|(_, items)| items).unwrap_or(&mut Vec::new()) {
                match item {
                    Item::Enum(item) => {
                        let (kind, docstr, flags) = match parse_attrs(&mut item.attrs) {
                            Ok((kind, docstr, flags)) => (kind, docstr, flags),
                            Err(err) => return err.into_compile_error().into()
                        };

                        if kind == TypeKind::Other { continue; }
                        if let Some(doc_override) = flags.doc_override {
                            items.push((DocItem::Override(item.ident.clone(), doc_override), kind, Vec::new()));
                            continue;
                        }

                        let mut variants = Vec::with_capacity(item.variants.len());

                        for variant in &mut item.variants {
                            let (variant_docs, flags) = match parse_attrs(&mut variant.attrs) {
                                Ok((_, variant_docs, flags)) => (variant_docs, flags),
                                Err(err) => return err.into_compile_error().into()
                            };

                            if !variant.fields.is_empty() {
                                return syn::Error::new(variant.span(), "enums with fields are unsupported!").into_compile_error().into()
                            }

                            if let Some((_, discriminant)) = &variant.discriminant {
                                if let Expr::Lit(ExprLit { lit: Lit::Int(lit), ..}) = discriminant {
                                    variants.push((variant.ident.clone(), Some(match lit.base10_parse::<u64>() { Ok(n) => n, Err(err) => return err.into_compile_error().into() }), variant_docs, flags))
                                } else {
                                    return syn::Error::new(discriminant.span(), "non-literal discriminants are unsupported!").into_compile_error().into()
                                }
                            } else {
                                variants.push((variant.ident.clone(), None, variant_docs, flags))
                            }
                        }

                        items.push((DocItem::Enum(item.ident.clone(), flags.rename.unwrap_or_else(|| item.ident.to_string()), variants), kind, docstr));
                    }
                    Item::Struct(item) => {
                        let (kind, docstr, flags) = match parse_attrs(&mut item.attrs) {
                            Ok((kind, docstr, flags)) => (kind, docstr, flags),
                            Err(err) => return err.into_compile_error().into()
                        };

                        if kind == TypeKind::Other { continue; }
                        if let Some(doc_override) = flags.doc_override {
                            items.push((DocItem::Override(item.ident.clone(), doc_override), kind, Vec::new()));
                            continue;
                        }

                        let mut fields = Vec::new();

                        for field in &mut item.fields {
                            let (field_docs, flags) = match parse_attrs(&mut field.attrs) {
                                Ok((_, field_docs, flags)) => (field_docs, flags),
                                Err(err) => return err.into_compile_error().into()
                            };

                            if let Some(ident) = &field.ident {
                                fields.push((ident.clone(), field.ty.clone(), field_docs, flags));
                            } else {
                                return syn::Error::new(field.span(), "tuple structs unsupported!").into_compile_error().into()
                            }
                        }

                        items.push((DocItem::Struct(item.ident.clone(), flags.rename.unwrap_or_else(|| item.ident.to_string()), fields), kind, docstr));
                    }
                    _ => {}
                }
            }

            let mut outfile = File::create("sde cheatsheet.yaml").unwrap();

            let intro = "\
            # Turtle's Cheat Sheet for the EVE Online 'Static Data Export'\n\
            #\n\
            # Each file in the Static Data Export is a mapping of the file's datatype to it's ID\n\
            # Field names with a trailing question mark `?` indicate an optional/nullable field that may not be present\n\
            #\n\
            # This cheat sheet is derived from code comments (https://github.com/SentientTurtle/EVE-3rd-party-dev-tools), and is structured accordingly.\n\
            # There will be more redundancy than conventional documentation. Some of the data structure is opinionated.\n\
            # Only the structure and types are validated automatically, the docs may be incomplete, shallowly researched, or outdated.\n\
            #\n\
            # This file is *NOT* valid YAML. Do not attempt to parse it, your parser will explode.\n\
            # If you have a good reason why it should become YAML-compliant or have an alternate notation for something, feel free to get in touch or open an issue in the repo above.\n\
            \n\
            # This file *may* be used as \"context\" for AI tools. Beware that it is a large file. I am not responsible if you delete your own wallet.
            \n\
            \n\
            # Common Data Types #\n\
            ";

            writeln!(outfile, "{}", intro).unwrap();

            fn write_item(outfile: &mut File, type_map: &HashMap<String, &(DocItem, TypeKind, Vec<String>)>, indent: usize, skip_type: bool, skip_key: bool, item: &DocItem, kind: &TypeKind, docstr: &Vec<String>) -> () {
                if !skip_type {
                    writeln!(outfile).unwrap();
                    if let TypeKind::SdeFile(filename) = kind {
                        write!(outfile, "{:indent$}## SDE File: {}.jsonl {}.yaml\n\n", "", filename, filename, indent = indent).unwrap();
                    }
                    for line in docstr {
                        writeln!(outfile, "{:indent$}# {}", "", line.trim(), indent = indent).unwrap();
                    }
                }
                match item {
                    DocItem::Enum(_, name, variants) => {
                        let discriminant_datatype = if let Some((_, Some(_), _, _)) = variants.first() {
                            "number"
                        } else {
                            "string"
                        };

                        if !skip_type {
                            writeln!(outfile, "{:indent$}{}: !!oneOf({})", "", name, discriminant_datatype, indent = indent).unwrap();
                        } else {
                            writeln!(outfile, " !!oneOf({})", discriminant_datatype).unwrap();
                        }
                        for (variant, discriminant, variant_docs, _flags) in variants {
                            for line in variant_docs {
                                writeln!(outfile, "{:indent$}# {}", "", line.trim(), indent=indent + 4).unwrap();
                            }
                            if let Some(discriminant) = discriminant {
                                writeln!(outfile, "{:indent$}- {}={}", "", variant, discriminant, indent=indent + 2).unwrap();
                            } else {
                                writeln!(outfile, "{:indent$}- {}", "", variant, indent=indent + 2).unwrap();
                            }
                        }
                    },
                    DocItem::Struct(_, name, fields) => {
                        if !skip_type {
                            writeln!(outfile, "{:indent$}{}:", "", name, indent = indent).unwrap();
                        } else {
                            writeln!(outfile).unwrap();
                        }
                        for (field, field_type, field_docs, flags) in fields {
                            if skip_key && flags.rename.as_deref() == Some("_key") {
                                continue;
                            }
                            let mut flags = flags.clone();
                            for line in field_docs {
                                writeln!(outfile, "{:indent$}# {}", "", line.trim(), indent=indent + 4).unwrap();
                            }

                            write_field_type(outfile, type_map, indent, field.to_string(), field_type, &mut flags);
                        }
                    },
                    DocItem::Override(_, doc_override) => {
                        if skip_type {
                            todo!("inline doc_sde(override) items are not supported!")
                        }
                        for line in doc_override.trim().lines() {
                            writeln!(outfile, "{:indent$}{}", "", line, indent=indent).unwrap();
                        }
                    }
                }
            }
            fn write_field_type(outfile: &mut File, type_map: &HashMap<String, &(DocItem, TypeKind, Vec<String>)>, indent: usize, field_name: String, field_type: &syn::Type, flags: &mut Flags) {
                let mut _type_alias = None; // Hold ownership of any alias generated below
                if let Some(alias) = flags.type_alias.take() {  // Unset alias if present, as this function is recursive and we handle the type alias in the current iteration
                    match syn::parse_str::<syn::Type>(&alias) {
                        Ok(t) => {
                            _type_alias = Some(t);
                        },
                        Err(_err) => {
                            todo!("invalid type in type_alias")
                        }
                    }
                }
                let field_type = _type_alias.as_ref().unwrap_or(field_type);

                let field_name = if let Some(alias) = &flags.rename && alias != "_key" {
                    alias.to_owned()    // Free borrow on flags, which is used mutably later
                } else {
                    field_name
                };

                #[allow(unused_qualifications)]
                if let syn::Type::Path(TypePath { qself: Option::None, path }) = field_type {
                    assert!(path.segments.len() <= 2, "{:?}", path);
                    if let Some(PathSegment { ident, arguments }) = path.segments.first() {
                        match &*ident.to_string() {
                            s @ ("f64" | "i32" | "u32" | "i64" | "u64" | "bool" | "String" | "ids" | "values" | "EVEUnit") => {
                                write!(outfile, "{:indent$}", "", indent = indent + 4).unwrap();
                                if flags.rename.as_deref() == Some("_key") { write!(outfile, "!!key ").unwrap() }
                                write!(outfile, "{}{}: ", field_name, if flags.nullable { "?" } else { "" }).unwrap();

                                match &flags.arity {
                                    Arity::Single => { /* NO-OP */}
                                    Arity::List => write!(outfile, "[ ").unwrap(),
                                    Arity::Map(key) => {
                                        write!(outfile, "{{ ").unwrap();

                                        #[allow(unused_qualifications)]
                                        if let syn::Type::Path(TypePath { qself: Option::None, path: key_path }) = key {
                                            assert!(key_path.segments.len() <= 2, "{:?}", key_path);
                                            if let Some(PathSegment { ident: key_ident, .. }) = key_path.segments.first() {
                                                write_simple_type(outfile, key_path, &key_ident.to_string())
                                                    .expect("non-simple map key!"); // TODO: Better error
                                            } else {
                                                todo!("Type without segments in path!?")
                                            }
                                        } else {
                                            todo!("Non-path type: {:?}", field_type)
                                        }

                                        write!(outfile, ": ").unwrap();
                                    }
                                }

                                write_simple_type(outfile, path, s).expect("simple_type must always be valid here");

                                match &flags.arity {
                                    Arity::Single => { /* NO-OP */ }
                                    Arity::List => write!(outfile, " ]").unwrap(),
                                    Arity::Map(_) => write!(outfile, " }}").unwrap(),
                                }
                                writeln!(outfile).unwrap();
                            }
                            "Option" => {
                                if let PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }) = arguments {
                                    if let Some(GenericArgument::Type(t)) = args.first() {
                                        flags.nullable = true;
                                        write_field_type(outfile, type_map, indent, field_name, t, flags);
                                    } else {
                                        todo!("Option with non-type argument")
                                    }
                                } else {
                                    todo!("argumentless Option")
                                }
                            }
                            "Vec" => {
                                if let PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }) = arguments {
                                    if let Some(GenericArgument::Type(t)) = args.first() {
                                        flags.arity = Arity::List;
                                        write_field_type(outfile, type_map, indent, field_name, t, flags);
                                    } else {
                                        todo!("Vec with non-type argument")
                                    }
                                } else {
                                    todo!("Argumentless Vec")
                                }
                            }
                            "IndexMap" => {
                                if let PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }) = arguments {
                                    if let (Some(GenericArgument::Type(k)), Some(GenericArgument::Type(v))) = (args.get(0), args.get(1)) {
                                        flags.arity = Arity::Map(k.clone());
                                        write_field_type(outfile, type_map, indent, field_name, v, flags);
                                    } else {
                                        todo!("{:?}", args)
                                    }
                                } else {
                                    todo!("Argumentless IndexMap")
                                }
                            }
                            s => {
                                match type_map.get(s) {
                                    Some((_, TypeKind::CommonType | TypeKind::ExternalType, _)) => {
                                        std::write!(outfile, "{:indent$}", "", indent = indent + 4).unwrap();
                                        if flags.rename.as_deref() == Some("_key") { write!(outfile, "!!key ").unwrap() }
                                        write!(outfile, "{}{}: ", field_name, if flags.nullable { "?" } else { "" }).unwrap();

                                        match &flags.arity {
                                            Arity::Single => { /* NO OP */ }
                                            Arity::List => write!(outfile, "[ ").unwrap(),
                                            Arity::Map(key) => {
                                                write!(outfile, "{{ ").unwrap();

                                                #[allow(unused_qualifications)]
                                                if let syn::Type::Path(TypePath { qself: Option::None, path: key_path }) = key {
                                                    assert!(key_path.segments.len() <= 2, "{:?}", key_path);
                                                    if let Some(PathSegment { ident: key_ident, .. }) = key_path.segments.first() {
                                                        write_simple_type(outfile, key_path, &key_ident.to_string())
                                                            .expect("non-simple map key!"); // TODO: Better error
                                                        write!(outfile, ": ").unwrap();
                                                    } else {
                                                        todo!("Type without segments in path!?")
                                                    }
                                                } else {
                                                    todo!("Non-path type: {:?}", field_type)
                                                }
                                            }
                                        }

                                        write!(outfile, "{}", s).unwrap();

                                        match &flags.arity {
                                            Arity::Single => { /* NO-OP */ }
                                            Arity::List => write!(outfile, " ]").unwrap(),
                                            Arity::Map(_) => write!(outfile, " }}").unwrap(),
                                        }
                                        writeln!(outfile).unwrap();
                                    },
                                    Some((_, TypeKind::InternalType, _)) => {
                                        std::write!(outfile, "{:indent$}", "", indent = indent + 4).unwrap();
                                        if flags.rename.as_deref() == Some("_key") { write!(outfile, "!!key ").unwrap() }
                                        write!(outfile, "{}{}:", field_name, if flags.nullable { "?" } else { "" }).unwrap();

                                        let mut plus_indent = 0;

                                        match &flags.arity {
                                            Arity::Single => { /* NO-OP */ }
                                            Arity::List => write!(outfile, " [").unwrap(),
                                            Arity::Map(key) => {
                                                write!(outfile, " {{\n").unwrap();

                                                #[allow(unused_qualifications)]
                                                if let syn::Type::Path(TypePath { qself: Option::None, path: key_path }) = key {
                                                    assert!(key_path.segments.len() <= 2, "{:?}", key_path);
                                                    if let Some(PathSegment { ident: key_ident, .. }) = key_path.segments.first() {
                                                        std::write!(outfile, "{:indent$}", "", indent = indent + 8).unwrap();
                                                        write_simple_type(outfile, key_path, &key_ident.to_string())
                                                            .expect("non-simple map key!"); // TODO: Better error
                                                        write!(outfile, ":").unwrap();

                                                        plus_indent += 4;
                                                    } else {
                                                        todo!("Type without segments in path!?")
                                                    }
                                                } else {
                                                    todo!("Non-path type: {:?}", field_type)
                                                }
                                            }
                                        }

                                        if let Some((inner_item, inner_kind, inner_docs)) = type_map.get(s) {
                                            write_item(outfile, type_map, indent + 4 + plus_indent, true, matches!(flags.arity, Arity::Map(_)), inner_item, inner_kind, inner_docs)
                                        } else {
                                            panic!("unknown internal type: {}", s)
                                        }

                                        match &flags.arity {
                                            Arity::Single => { /* NO-OP */ }
                                            Arity::List => write!(outfile, "{:indent$}]\n", "", indent = indent + 4).unwrap(),
                                            Arity::Map(_) => write!(outfile, "{:indent$}}}\n", "", indent = indent + 4).unwrap(),
                                        }
                                        // writeln!(outfile).unwrap();
                                    }
                                    _ => {
                                        writeln!(outfile, "{:indent$}{}{}: UNKNOWN TYPE: {}", "", field_name, if flags.nullable { "?" } else { "" }, s, indent = indent + 4).unwrap()
                                    },
                                }
                            }
                        }
                    } else {
                        todo!("Type without segments in path!?")
                    }
                } else {
                    todo!("Non-path type: {:?}", field_type)
                }
            }
            fn write_simple_type(outfile: &mut File, path: &Path, s: &str) -> Result<(), ()> {
                match s {
                    "f64" => write!(outfile, "number").unwrap(),
                    "i32" | "u32" => write!(outfile, "integer").unwrap(),
                    "i64" | "u64" => write!(outfile, "64-bit long integer").unwrap(),
                    "bool" => write!(outfile, "boolean").unwrap(),
                    "String" => write!(outfile, "string").unwrap(),
                    "ids" => {
                        if path.segments.len() == 2 {
                            write!(outfile, "{} (integer)", path.segments[1].ident).unwrap()
                        } else {
                            todo!("ids:: without 2nd segment")
                        }
                    }
                    "EVEUnit" => write!(outfile, "UnitID (integer)").unwrap(),
                    "values" => {
                        if path.segments.len() == 2 {
                            let value_type = path.segments[1].ident.to_string();

                            let description = match &*value_type {
                                "SkillLevel" => " (integer [1, 5])",
                                "MetaLevel" => " (integer)",
                                "CacheResource" => "",
                                _ => todo!("Unknown value:: type `{}`", value_type)
                            };

                            write!(outfile, "{}{}", value_type, description).unwrap()
                        } else {
                            todo!("ids:: without 2nd segment")
                        }
                    }
                    _ => return Err(())
                }
                Ok(())
            }

            let map: HashMap<String, &(DocItem, TypeKind, Vec<String>)> = items.iter().map(|item| (item.0.ident().to_string(), item)).collect();
            // Write common types first
            for (item, kind, docstr) in &items {
                if let TypeKind::CommonType = kind {
                    write_item(&mut outfile, &map, 0, false, false, item, kind, docstr);
                    writeln!(outfile).unwrap();
                }
            }

            for (item, kind, docstr) in &items {
                if let TypeKind::SdeFile(_) | TypeKind::ExternalType = kind {
                    write_item(&mut outfile, &map, 0, false, false, item, kind, docstr);
                    writeln!(outfile).unwrap();
                }
            }

            module.to_token_stream().into()
        },
        Err(err) => syn::Error::new(err.span(), "export_docs must be used on a module")
            .to_compile_error()
            .into()
    }
}
