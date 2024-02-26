use lazy_static::lazy_static;
use proc_macro as pm;
use quote::{quote, ToTokens};
use syn::{
    parse::{ParseStream, Parser, Result},
    parse_macro_input,
    punctuated::Punctuated,
    Ident, Item, LitInt, MetaList, Path, PathSegment, Token,
};

const API_LEVEL_NEXT: u32 = 0xFFFFFFFE;
const API_LEVEL_HEAD: u32 = 0xFFFFFFFF;
const API_LEVEL_MIN: u32 = 10;
const API_LEVEL_MAX: u32 = 20;

const _NUMBERED_LEVELS: usize = (API_LEVEL_MAX - API_LEVEL_MIN + 1) as usize;
const _NAMED_LEVELS: usize = 2;

lazy_static! {
    static ref API_LEVELS: [u32; _NUMBERED_LEVELS + _NAMED_LEVELS] = core::array::from_fn(|i| {
        if i < _NUMBERED_LEVELS {
            i as u32 + API_LEVEL_MIN
        } else {
            0xFFFFFFFF - (i - _NUMBERED_LEVELS) as u32
        }
    });
}

/// For a numeric API level return the Rust cfg string that will be set when compiling to target support for that API level.
fn level_cfg(level: u32) -> String {
    match level {
        API_LEVEL_NEXT => "head".to_owned(),
        API_LEVEL_HEAD => "next".to_owned(),
        API_LEVEL_MIN..=API_LEVEL_MAX => format!("{level}"),
        _ => panic!("Unexpected API level {level}"),
    }
}

fn ident(ident: &str) -> Ident {
    Ident::new(ident, proc_macro2::Span::call_site())
}

#[derive(Default, Debug)]
struct Availability {
    pub added: Option<u32>,
    pub removed: Option<u32>,
}

impl Availability {
    /// Does this not actually specify any availability information?
    fn is_empty(&self) -> bool {
        self.added.is_none() && self.removed.is_none()
    }
    fn supported_levels(&self) -> Vec<String> {
        API_LEVELS
            .iter()
            .filter(|&&level| {
                if let Some(added) = self.added {
                    if level < added {
                        return false;
                    }
                }
                if let Some(removed) = self.removed {
                    if level >= removed {
                        return false;
                    }
                }
                return true;
            })
            .map(|&level| level_cfg(level))
            .collect()
    }

    fn cfg_args(&self) -> proc_macro2::TokenStream {
        let mut level_list: Punctuated<Ident, Token![,]> = Default::default();
        for level_str in self.supported_levels().into_iter() {
            level_list.push(ident(&format!("fuchsia_api_level_{level_str}")));
        }

        proc_macro2::TokenStream::from(quote!(any(#level_list)))
    }
}

#[derive(Default, Debug)]
struct AvailableArgsParser;

impl Parser for AvailableArgsParser {
    type Output = Availability;

    fn parse2(self, tokens: proc_macro2::TokenStream) -> Result<Self::Output> {
        let mut availability: Availability = Default::default();

        let parser = syn::meta::parser(|meta| {
            let parse_api_level = |tokens: ParseStream| {
                if let Ok(lit_int) = tokens.parse::<LitInt>() {
                    lit_int.base10_parse::<u32>()
                } else if let Ok(ident) = tokens.parse::<Ident>() {
                    if ident == "HEAD" {
                        Ok(API_LEVEL_HEAD)
                    } else if ident == "NEXT" {
                        Ok(API_LEVEL_NEXT)
                    } else {
                        Err(meta.error("Invalid API level"))
                    }
                } else {
                    Err(meta.error("Invalid API level"))
                }
            };
            if meta.path.is_ident("added") {
                availability.added = Some(parse_api_level(meta.value()?)?);
                Ok(())
            } else if meta.path.is_ident("removed") {
                availability.removed = Some(parse_api_level(meta.value()?)?);
                Ok(())
            } else {
                Err(meta.error("unsupported available property"))
            }
        });

        parser.parse2(tokens)?;
        Ok(availability)
    }
}

fn cfg_path() -> Path {
    let mut cfg_segments: Punctuated<PathSegment, syn::token::PathSep> = Default::default();
    cfg_segments.push(PathSegment {
        ident: Ident::new("cfg", proc_macro2::Span::call_site()),
        arguments: Default::default(),
    });
    Path {
        leading_colon: None,
        segments: cfg_segments,
    }
}

struct AvailableVisitor;
impl syn::visit_mut::VisitMut for AvailableVisitor {
    fn visit_attribute_mut(&mut self, attr: &mut syn::Attribute) {
        if let syn::Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("available") {
                let availability = attr.parse_args_with(AvailableArgsParser).unwrap();

                let cfg_ml = MetaList {
                    path: cfg_path(),
                    delimiter: meta_list.delimiter.clone(),
                    tokens: availability.cfg_args(),
                };
                attr.meta = syn::Meta::List(cfg_ml);
            }
        }
    }
}

#[proc_macro_attribute]
pub fn available(args: pm::TokenStream, item: pm::TokenStream) -> pm::TokenStream {
    // Process all of the #[available()] inside this item
    let mut input = parse_macro_input!(item as Item);
    syn::visit_mut::visit_item_mut(&mut AvailableVisitor, &mut input);

    let availability = parse_macro_input!(args with AvailableArgsParser);
    if availability.is_empty() {
        // If no availability was specified at the top-level, just return the block
        pm::TokenStream::from(input.to_token_stream())
    } else {
        // Generate a #[cfg(...)] macro invocation before the item
        let cfg_args = availability.cfg_args();

        pm::TokenStream::from(quote!(
            #[cfg(#cfg_args)]
            #input))
    }
}
