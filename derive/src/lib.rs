extern crate proc_macro;
use std::collections::HashSet;

#[proc_macro_derive(ServiceDef, attributes(cli, conf))]
pub fn derive_service_def(stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut parsed = syn::parse_macro_input!(stream as syn::Item);
    match &mut parsed {
        syn::Item::Struct(item) => impl_mod_struct(item),
        _ => {
            let error = syn::Error::new(proc_macro2::Span::call_site(), "not a struct type");
            error.to_compile_error()
        }
    }
    .into()
}

fn impl_mod_struct(item: &syn::ItemStruct) -> proc_macro2::TokenStream {
    let attrs = parse_attributes(&item.attrs);
    let service = item.ident.clone();
    let generics = item.generics.clone();
    impl_mod(attrs, service, generics)
}

fn impl_mod(
    attrs: HashSet<DefParam>,
    name: syn::Ident,
    generics: syn::Generics,
) -> proc_macro2::TokenStream {
    let mut impl_cli = quote::quote!();
    let mut impl_conf = quote::quote!(
        #[derive(Default, ::serde::Deserialize)]
        pub struct Conf {}
    );

    for attr in attrs {
        match attr {
            DefParam::Cli(ident) => {
                impl_cli = quote::quote!(
                    #[structopt(flatten)]
                    pub service: super::#ident,
                );
            }
            DefParam::Conf(ident) => {
                impl_conf = quote::quote!(
                    pub type Conf = super::#ident;
                )
            }
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote::quote!(

        #[doc(hidden)]
        pub mod ya_service_sdk_impl {
            #[derive(structopt::StructOpt)]
            #[structopt(setting = structopt::clap::AppSettings::ColoredHelp)]
            #[structopt(setting = structopt::clap::AppSettings::DeriveDisplayOrder)]
            #[structopt(setting = structopt::clap::AppSettings::VersionlessSubcommands)]
            pub struct Cli {
                /// Working directory
                #[structopt(short, long)]
                #[structopt(required_ifs(&[
                    ("command", "deploy"),
                    ("command", "start"),
                    ("command", "run"),
                ]))]
                pub workdir: Option<std::path::PathBuf>,

                #impl_cli

                /// Command to execute
                #[structopt(subcommand)]
                pub command: ::ya_service_sdk::cli::Command,
            }

            impl ::ya_service_sdk::cli::CommandCli for Cli {
                fn workdir(&self) -> Option<std::path::PathBuf> {
                    self.workdir.clone()
                }

                fn command(&self) -> &::ya_service_sdk::cli::Command {
                    &self.command
                }
            }

            #impl_conf
        }

        impl #impl_generics ::ya_service_sdk::ServiceDef for #name #ty_generics #where_clause {
            const NAME: &'static str = env!("CARGO_PKG_NAME");
            const VERSION: &'static str = env!("CARGO_PKG_VERSION");

            type Cli = ya_service_sdk_impl::Cli;
            type Conf = ya_service_sdk_impl::Conf;
        }
    )
}

fn parse_attributes(attrs: &Vec<syn::Attribute>) -> HashSet<DefParam> {
    attrs
        .iter()
        .map(|attr| (attr, attr.path.segments[0].ident.to_string()))
        .filter(|(_, variant)| {
            DefParam::VARIANTS
                .iter()
                .position(|v| *v == variant.as_str())
                .is_some()
        })
        .map(|(attr, variant)| {
            let ident = syn::parse2::<DefIdent>(attr.tokens.clone())
                .expect(&format!("invalid value {}", variant));
            DefParam::new(&variant, ident.0)
        })
        .collect()
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum DefParam {
    Cli(syn::Ident),
    Conf(syn::Ident),
}

impl DefParam {
    const VARIANTS: [&'static str; 2] = ["cli", "conf"];

    fn new(variant: &String, ident: syn::Ident) -> Self {
        match variant.as_str() {
            "cli" => DefParam::Cli(ident),
            "conf" => DefParam::Conf(ident),
            _ => panic!("invalid attribute: {}", variant),
        }
    }
}

struct DefIdent(syn::Ident);

impl syn::parse::Parse for DefIdent {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        let item = content.parse()?;
        Ok(DefIdent(item))
    }
}
