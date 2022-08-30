use darling::{FromDeriveInput, Result};
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::parse_quote;

pub fn standalone(input: syn::DeriveInput) -> Result<TokenStream> {
    StandaloneImpl::for_struct(input).map(ToTokens::into_token_stream)
}

#[derive(FromDeriveInput)]
#[darling(
    attributes(message, standalone),
    supports(struct_named, struct_unit, struct_tuple, struct_newtype)
)]
struct StandaloneParams {
    mod_path: Option<syn::Path>,
    #[darling(rename = "id")]
    id: u8,
    ident: syn::Ident,
    generics: syn::Generics,
}

impl StandaloneParams {
    fn full_trait_path(&self) -> syn::Path {
        super::full_item_path(
            &self.mod_path,
            super::MOD_PATH,
            super::STANDALONE_TRAIT_NAME,
        )
    }
}

struct StandaloneImpl {
    impl_block: syn::ItemImpl,
}

impl StandaloneImpl {
    fn for_struct(input: syn::DeriveInput) -> Result<Self> {
        let params = <StandaloneParams as FromDeriveInput>::from_derive_input(&input)?;

        let trait_path = params.full_trait_path();

        let StandaloneParams {
            id,
            ident,
            generics,
            ..
        } = params;
        let (impl_gens, ty_gens, where_clause) = generics.split_for_impl();

        let impl_block = parse_quote! {
            impl #impl_gens #trait_path for #ident #ty_gens #where_clause {
                const ID: u8 = #id;
            }
        };

        Ok(Self { impl_block })
    }
}

impl ToTokens for StandaloneImpl {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.impl_block.to_tokens(tokens)
    }
}
