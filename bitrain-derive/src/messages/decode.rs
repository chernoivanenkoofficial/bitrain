use darling::{ast::Data, util::Ignored, Error, FromDeriveInput, Result, ToTokens};
use proc_macro2::TokenStream;
use quote::format_ident;
use syn::{parse_quote, DeriveInput};

pub fn decode(input: DeriveInput) -> Result<TokenStream> {
    DecodeImpl::for_struct(input).map(ToTokens::into_token_stream)
}

#[derive(darling::FromDeriveInput)]
#[darling(
    attributes(message),
    supports(struct_named, struct_unit, struct_tuple, struct_newtype)
)]
struct DecodeParams {
    mod_path: Option<syn::Path>,
    ident: syn::Ident,
    generics: syn::Generics,
    data: Data<Ignored, super::Field>,
}

impl DecodeParams {
    fn full_trait_path(&self) -> syn::Path {
        super::full_item_path(&self.mod_path, super::MOD_PATH, super::DECODE_TRAIT_NAME)
    }
}

struct DecodeFromCall {
    call: syn::Stmt,
}

impl DecodeFromCall {
    fn from_struct_field(
        (pos, field): (usize, &super::Field),
        trait_path: &syn::Path,
    ) -> Result<Self> {
        let var_name = struct_field_name((pos, field));
        let field_type = &field.ty;

        let call: syn::Stmt = parse_quote! {
            let #var_name = if let Some(val) = <#field_type as #trait_path>::decode_from(
                len_hint,
                reader
            )? {
                val
            } else {
                return Ok(None)
            };
        };

        Ok(Self { call })
    }
}

impl ToTokens for DecodeFromCall {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.call.to_tokens(tokens)
    }
}

struct SelfInit {
    init: syn::Expr,
}

impl SelfInit {
    fn from_struct_fields(params: &DecodeParams) -> Result<Self> {
        let fields = params.data.as_ref().take_struct().unwrap();

        let underscored = fields
            .iter()
            .enumerate()
            .map(|(pos, field)| struct_field_name((pos, *field)));

        let init: syn::Expr = if fields.is_tuple() {
            parse_quote!(
                Ok(
                    Some(
                        Self(#(#underscored,)*)
                    )
                )
            )
        } else {
            let field_names = fields.iter().map(|field| field.ident.as_ref().unwrap());

            parse_quote!(
                Ok(
                    Some(
                        Self {
                            #(#field_names: #underscored,)*
                        }
                    )
                )
            )
        };

        Ok(Self { init })
    }
}

impl ToTokens for SelfInit {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.init.to_tokens(tokens)
    }
}

struct DecodeFromDef {
    fn_def: syn::ItemFn,
}

impl DecodeFromDef {
    fn from_struct_fields<'a>(params: &DecodeParams) -> Result<Self> {
        let fields = params.data.as_ref().take_struct().unwrap();
        let mut errors = Error::accumulator();

        let inner_calls = fields
            .iter()
            .enumerate()
            .map(|(pos, field)| {
                DecodeFromCall::from_struct_field((pos, *field), &params.full_trait_path())
            })
            .filter_map(|result| errors.handle(result))
            .collect::<Vec<_>>();

        errors.finish()?;

        let self_init = SelfInit::from_struct_fields(params)?;

        let fn_def: syn::ItemFn = parse_quote! {
            fn decode_from(
                len_hint: &mut usize,
                reader: &mut impl ::std::io::Read
            ) -> ::std::io::Result<::std::option::Option<Self>> {
                #(#inner_calls)*

                #self_init
            }
        };

        Ok(Self { fn_def })
    }
}

impl ToTokens for DecodeFromDef {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.fn_def.to_tokens(tokens)
    }
}

struct DecodeImpl {
    impl_block: syn::ItemImpl,
}

impl DecodeImpl {
    fn for_struct(input: DeriveInput) -> Result<Self> {
        let mut params: DecodeParams = FromDeriveInput::from_derive_input(&input)?;

        let decode_from_def = DecodeFromDef::from_struct_fields(&params)?;
        let trait_path = params.full_trait_path();
        Self::adjust_generics(&mut params);

        let DecodeParams {
            ident, generics, ..
        } = params;

        let (impl_gens, ty_gens, where_clause) = generics.split_for_impl();

        let impl_block: syn::ItemImpl = parse_quote! {
            #[automatically_derived]
            impl #impl_gens #trait_path for #ident #ty_gens #where_clause {
                #decode_from_def
            }
        };

        Ok(Self { impl_block })
    }

    fn adjust_generics(meta: &mut DecodeParams) {
        use crate::ast::bounds::Bind;

        let bound: syn::TraitBound = syn::parse2(meta.full_trait_path().to_token_stream()).unwrap();
        //TODO: Move generic bound directly to underlying type
        meta.generics.params.bind_all(Some(bound));
    }
}

impl ToTokens for DecodeImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.impl_block.to_tokens(tokens)
    }
}

fn struct_field_name((pos, field): (usize, &super::Field)) -> syn::Ident {
    match field.ident.as_ref() {
        Some(ident) => ident.to_owned(),
        None => {
            format_ident!("__decoded_{}", pos)
        }
    }
}
