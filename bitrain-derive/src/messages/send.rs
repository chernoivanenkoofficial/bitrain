use darling::{
    ast::{Data, Fields, Style},
    util::Ignored,
    Error, FromDeriveInput, FromVariant, Result
};
use proc_macro2::TokenStream;
use syn::{parse_quote, punctuated::Punctuated, DeriveInput};

pub fn send(input: syn::DeriveInput) -> Result<TokenStream> {
    SendImpl::for_enum(&input).map(quote::ToTokens::into_token_stream)
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(message), supports(enum_any))]
struct SendParams {
    mod_path: Option<syn::Path>,
    ident: syn::Ident,
    generics: syn::Generics,
    data: Data<SendVariant, Ignored>,
}

impl SendParams {
    fn encode_trait_path(&self) -> syn::Path {
        super::full_item_path(&self.mod_path, super::MOD_PATH, super::ENCODE_TRAIT_NAME)
    }

    fn send_trait_path(&self) -> syn::Path {
        super::full_item_path(&self.mod_path, super::MOD_PATH, super::SEND_TRAIT_NAME)
    }

    fn container_struct_path(&self) -> syn::Path {
        super::full_item_path(
            &self.mod_path,
            super::MOD_PATH,
            super::CONTAINER_STRUCT_NAME,
        )
    }
}

#[derive(Debug, FromVariant)]
#[darling(attributes(standalone), and_then = "SendVariant::validate")]
struct SendVariant {
    ident: syn::Ident,
    fields: Fields<super::Field>,
    id: Option<u8>,
}

impl SendVariant {
    fn validate(self) -> Result<Self> {
        if self.id.is_none() && self.fields.style.is_unit() {
            return Err(Error::missing_field("id"));
        }

        Ok(self)
    }
}

struct SendToMatchArm {
    match_arm: syn::Arm,
}

impl SendToMatchArm {
    fn from_variant(
        variant: &SendVariant,
        send_trait_path: &syn::Path,
        container_struct_path: &syn::Path,
        encode_trait_path: &syn::Path,
    ) -> Result<Self> {
        let match_arm: syn::Arm = match variant.fields.style {
            Style::Struct => {
                if variant.fields.fields.len() != 1 {
                    return Err(Error::unsupported_shape(
                        "Not single field in associated data.",
                    ));
                }

                let variant_ident = &variant.ident;
                let struct_ident = &variant.fields.fields[0].ident.to_owned().unwrap();

                parse_quote! {
                    Self::#variant_ident { #struct_ident } => {
                        #send_trait_path::send_to(&#container_struct_path(#struct_ident), writer)
                    }
                }
            }
            Style::Tuple => {
                if variant.fields.fields.len() != 1 {
                    return Err(Error::unsupported_shape(
                        "Not single field in associated data.",
                    ));
                }

                let variant_ident = &variant.ident;

                parse_quote! {
                    Self::#variant_ident(data) => {
                        #send_trait_path::send_to(&#container_struct_path(data), writer)
                    }
                }
            }
            Style::Unit => {
                if variant.id.is_none() {
                    return Err(Error::missing_field(
                        r#"Unit variants should specify id explicitly via 
                        #[standalone(id = 'id_value')]"#,
                    ));
                }

                let variant_ident = &variant.ident;
                let id = variant.id.to_owned().unwrap();

                parse_quote! {
                    Self::#variant_ident => {
                        <u32 as #encode_trait_path>::encode_to(&1u32, writer)?;
                        <u8 as #encode_trait_path>::encode_to(&#id, writer)
                    }
                }
            }
        };

        Ok(Self { match_arm })
    }
}

impl quote::ToTokens for SendToMatchArm {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.match_arm.to_tokens(tokens)
    }
}

struct SendToDef {
    fn_def: syn::ItemFn,
}

impl SendToDef {
    fn from_params(params: &SendParams) -> Result<Self> {
        let encode_trait_path = params.encode_trait_path();
        let send_trait_path = params.send_trait_path();
        let container_struct_path = params.container_struct_path();

        let mut errors = Error::accumulator();

        let match_arms = params
            .data
            .as_ref()
            .take_enum()
            .unwrap()
            .into_iter()
            .map(|var| {
                SendToMatchArm::from_variant(
                    var,
                    &send_trait_path,
                    &container_struct_path,
                    &encode_trait_path,
                )
            })
            .filter_map(|res| errors.handle(res))
            .collect::<Vec<_>>();

        errors.finish()?;

        let fn_def: syn::ItemFn = parse_quote! {
            fn send_to(&self, writer: &mut impl ::std::io::Write) -> ::std::io::Result<()> {
                match self {
                    #(#match_arms,)*
                }
            }
        };

        Ok(Self { fn_def })
    }
}

impl quote::ToTokens for SendToDef {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.fn_def.to_tokens(tokens)
    }
}

struct SendImpl {
    impl_block: syn::ItemImpl,
}

impl SendImpl {
    fn for_enum(input: &DeriveInput) -> Result<Self> {
        let mut params = <SendParams as FromDeriveInput>::from_derive_input(&input)?;

        let send_to_def = SendToDef::from_params(&params)?;
        let send_trait_path = params.send_trait_path();

        Self::adjust_generics(&mut params)?;

        let SendParams {
            ident, generics, ..
        } = params;

        let (impl_gens, ty_gens, where_clause) = generics.split_for_impl();

        let impl_block = parse_quote! {
            #[automatically_derived]
            impl #impl_gens #send_trait_path for #ident #ty_gens #where_clause {
                #send_to_def
            }
        };

        Ok(Self { impl_block })
    }

    fn adjust_generics(params: &mut SendParams) -> Result<()> {
        let mut bounds = Punctuated::new();
        bounds.push(
            syn::TraitBound {
                lifetimes: None,
                modifier: syn::TraitBoundModifier::None,
                paren_token: None,
                path: params.encode_trait_path(),
            }
            .into(),
        );

        params
            .data
            .as_ref()
            .take_enum()
            .unwrap()
            .iter()
            .filter_map(|&var| var.fields.fields.first().map(|f| &f.ty))
            .for_each(|ty| {
                let predicate = syn::PredicateType {
                    bounded_ty: ty.clone(),
                    bounds: bounds.clone(),
                    colon_token: Default::default(),
                    lifetimes: None,
                };

                params
                    .generics
                    .make_where_clause()
                    .predicates
                    .push(predicate.into())
            });

        Ok(())
    }
}

impl quote::ToTokens for SendImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.impl_block.to_tokens(tokens)
    }
}
