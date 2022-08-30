use darling::ast::{Data, Fields};
use darling::util::Ignored;
use darling::{Error, FromDeriveInput, Result};

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::parse_quote;

pub fn encode(container: syn::DeriveInput) -> Result<TokenStream> {
    EncodeImpl::for_struct(container).map(ToTokens::into_token_stream)
}

#[derive(darling::FromDeriveInput)]
#[darling(
    attributes(message),
    supports(struct_named, struct_unit, struct_tuple, struct_newtype)
)]
struct EncodeParams {
    ident: syn::Ident,
    generics: syn::Generics,
    data: Data<Ignored, super::Field>,
    mod_path: Option<syn::Path>,
}

impl EncodeParams {
    fn full_trait_path(&self) -> syn::Path {
        super::full_item_path(&self.mod_path, super::MOD_PATH, super::ENCODE_TRAIT_NAME)
    }

    fn fields(&self) -> Option<Fields<&super::Field>> {
        self.data.as_ref().take_struct()
    }
}

struct EncodeToCall {
    call: syn::Stmt,
}

impl EncodeToCall {
    fn from_field((pos, field): (usize, &super::Field), trait_path: &syn::Path) -> Result<Self> {
        let call = if let Some(ident) = &field.ident {
            parse_quote! {
                #trait_path::encode_to((&self.#ident).deref(), writer)?;
            }
        } else {
            let index = syn::Index::from(pos);

            parse_quote! {
                #trait_path::encode_to((&self.#index).deref(), writer)?;
            }
        };

        Ok(Self { call })
    }
}

impl ToTokens for EncodeToCall {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.call.to_tokens(tokens)
    }
}

struct EncodeToDef {
    fn_def: syn::ItemFn,
}

impl EncodeToDef {
    fn from_fields<'a>(params: &EncodeParams) -> Result<Self> {
        let mut errors = Error::accumulator();

        let fields = params.fields().unwrap();
        let trait_path = params.full_trait_path();

        let inner_calls = fields
            .into_iter()
            .enumerate()
            .map(|arg| EncodeToCall::from_field(arg, &trait_path))
            .filter_map(|result| errors.handle(result))
            .collect::<Vec<_>>();

        errors.finish()?;

        Self::new(inner_calls.iter())
    }

    fn new<'a>(inner_calls: impl IntoIterator<Item = &'a EncodeToCall>) -> Result<Self> {
        let inner_calls = inner_calls.into_iter();

        let fn_def = parse_quote! {
            fn encode_to(&self, writer: &mut impl ::std::io::Write) -> ::std::io::Result<()> {
                #(#inner_calls)*

                Ok(())
            }
        };

        Ok(Self { fn_def })
    }
}

impl ToTokens for EncodeToDef {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.fn_def.to_tokens(tokens)
    }
}

struct SizeCall {
    size_call: syn::Expr,
}

impl SizeCall {
    fn from_field((pos, field): (usize, &super::Field), trait_path: &syn::Path) -> Result<Self> {
        let size_call = if let Some(ident) = &field.ident {
            parse_quote!(
                #trait_path::size((&self.#ident).deref())
            )
        } else {
            let index = syn::Index::from(pos);

            parse_quote!(
                #trait_path::size((&self.#index).deref())
            )
        };

        Ok(Self { size_call })
    }
}

impl ToTokens for SizeCall {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.size_call.to_tokens(tokens)
    }
}

struct SizeDef {
    fn_def: syn::ItemFn,
}

impl SizeDef {
    fn from_params<'a>(params: &EncodeParams) -> Result<Self> {
        let mut errors = Error::accumulator();

        let fields = params.data.as_ref().take_struct().unwrap();
        let trait_path = params.full_trait_path();

        let inner_calls = fields
            .into_iter()
            .enumerate()
            .map(|arg| SizeCall::from_field(arg, &trait_path))
            .filter_map(|result| errors.handle(result))
            .collect::<Vec<_>>();

        errors.finish()?;

        Self::new(inner_calls.iter())
    }

    fn new<'a>(inner_calls: impl IntoIterator<Item = &'a SizeCall>) -> Result<Self> {
        let inner_calls = inner_calls.into_iter();

        let fn_def = parse_quote! {
            fn size(&self) -> usize {
                #(#inner_calls +)* 0usize
            }
        };

        Ok(Self { fn_def })
    }
}

impl ToTokens for SizeDef {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.fn_def.to_tokens(tokens)
    }
}

struct EncodeImpl {
    impl_block: syn::Item,
}

impl EncodeImpl {
    fn for_struct(input: syn::DeriveInput) -> Result<Self> {
        let mut params: EncodeParams = FromDeriveInput::from_derive_input(&input)?;

        let encode_to_def = EncodeToDef::from_fields(&params)?;
        let size_def = SizeDef::from_params(&params)?;

        Self::adjust_generics(&mut params);
        let trait_path = params.full_trait_path();

        let EncodeParams {
            ident, generics, ..
        } = params;

        let (impl_gens, ty_gens, where_clause) = generics.split_for_impl();

        let impl_block = parse_quote! {
            #[automatically_derived]
            impl #impl_gens #trait_path for #ident #ty_gens #where_clause {
                #encode_to_def
                #size_def
            }
        };

        Ok(Self { impl_block })
    }

    fn adjust_generics(params: &mut EncodeParams) -> () {
        use crate::ast::bounds::Bind;

        let bound: syn::TraitBound =
            syn::parse2(params.full_trait_path().to_token_stream()).unwrap();

        //TODO: Move generic bound directly to underlying type
        params.generics.params.bind_all(Some(bound));
    }
}

impl ToTokens for EncodeImpl {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.impl_block.to_tokens(tokens)
    }
}
