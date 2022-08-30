use darling::{
    ast::{Data, Fields, Style},
    util::Ignored,
    Error, FromDeriveInput, FromVariant, Result,
};
use proc_macro2::TokenStream;
use syn::{parse_quote, punctuated::Punctuated, DeriveInput};

pub fn recv(input: syn::DeriveInput) -> Result<TokenStream> {
    RecvImpl::for_enum(&input).map(quote::ToTokens::into_token_stream)
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(message), supports(enum_any))]
struct RecvParams {
    mod_path: Option<syn::Path>,
    ident: syn::Ident,
    generics: syn::Generics,
    data: Data<RecvVariant, Ignored>,
}

impl RecvParams {
    fn decode_trait_path(&self) -> syn::Path {
        super::full_item_path(&self.mod_path, super::MOD_PATH, super::DECODE_TRAIT_NAME)
    }

    fn standalone_trait_path(&self) -> syn::Path {
        super::full_item_path(
            &self.mod_path,
            super::MOD_PATH,
            super::STANDALONE_TRAIT_NAME,
        )
    }

    fn recv_trait_path(&self) -> syn::Path {
        super::full_item_path(&self.mod_path, super::MOD_PATH, super::RECV_TRAIT_NAME)
    }
}

#[derive(Debug, FromVariant)]
#[darling(attributes(standalone), and_then = "RecvVariant::validate")]
struct RecvVariant {
    ident: syn::Ident,
    fields: Fields<super::Field>,
    id: Option<u8>,
}

impl RecvVariant {
    fn validate(self) -> Result<Self> {
        if self.id.is_none() && self.fields.style.is_unit() {
            return Err(Error::missing_field("id"));
        }

        Ok(self)
    }
}

struct RecvFromMatchArm {
    match_arm: syn::Arm,
}

impl RecvFromMatchArm {
    fn from_variant(
        variant: &RecvVariant,
        standalone_trait_path: &syn::Path,
        decode_trait_path: &syn::Path,
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
                let ty = &variant.fields.fields[0].ty;

                parse_quote! {
                    <#ty as #standalone_trait_path>::ID => {
                        let #struct_ident = <#ty as #decode_trait_path>::decode_or_discard_from(
                            &mut len_hint, 
                            reader
                        )?;
                        #struct_ident.map(|#struct_ident| Self::#variant_ident { #struct_ident })
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
                let ty = &variant.fields.fields[0].ty;

                parse_quote! {
                    <#ty as #standalone_trait_path>::ID => {
                        let data = <#ty as #decode_trait_path>::decode_or_discard_from(
                            &mut len_hint, 
                            reader
                        )?;
                        data.map(Self::#variant_ident)
                    }
                }
            }
            Style::Unit => {
                if variant.id.is_none() {
                    return Err(Error::missing_field(
                        r#"Unit variants should specify id explicitly via 
                    #[standalone(id = 'id_value')] or have corresponding discriminant"#,
                    ));
                }

                let variant_ident = &variant.ident;
                let id = variant.id.to_owned().unwrap();

                parse_quote! {
                    #id => Some(Self::#variant_ident)
                }
            }
        };

        Ok(Self { match_arm })
    }
}

impl quote::ToTokens for RecvFromMatchArm {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.match_arm.to_tokens(tokens)
    }
}

struct RecvFromDef {
    fn_def: syn::ItemFn,
}

impl RecvFromDef {
    fn from_params(params: &RecvParams) -> Result<Self> {
        let decode_trait_path = params.decode_trait_path();
        let standalone_trait_path = params.standalone_trait_path();

        let mut errors = Error::accumulator();

        let match_arms = params
            .data
            .as_ref()
            .take_enum()
            .unwrap()
            .into_iter()
            .map(|var| {
                RecvFromMatchArm::from_variant(var, &standalone_trait_path, &decode_trait_path)
            })
            .filter_map(|res| errors.handle(res))
            .collect::<Vec<_>>();

        errors.finish()?;

        let fn_def: syn::ItemFn = parse_quote! {
            fn recv_from(reader: &mut impl ::std::io::Read) -> ::std::io::Result<::std::option::Option<Self>> {
                let mut len_hint = if let Some(val) = <u32 as #decode_trait_path>::decode_or_discard_from(
                    &mut ::std::mem::size_of::<u32>(), 
                    reader
                )? {
                    val as usize
                } else {
                    return Ok(None)
                };

                if len_hint == 0 {
                    return Ok(None)
                }

                let id = if let Some(val) = <u8 as #decode_trait_path>::decode_or_discard_from(
                    &mut ::std::mem::size_of::<u8>(), 
                    reader
                )? {
                    val
                } else {
                    return Ok(None)
                };

                len_hint -= 1;

                let message = match id {
                    #(#match_arms,)*
                    _ => None
                };
                
                Ok(message)
            }
        };

        Ok(Self { fn_def })
    }   
}

impl quote::ToTokens for RecvFromDef {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.fn_def.to_tokens(tokens)
    }
}

struct RecvImpl {
    impl_block: syn::ItemImpl,
}

impl RecvImpl {
    fn for_enum(input: &DeriveInput) -> Result<Self> {
        let mut params = <RecvParams as FromDeriveInput>::from_derive_input(&input)?;

        let recv_from_def = RecvFromDef::from_params(&params)?;
        let recv_trait_path = params.recv_trait_path();

        Self::adjust_generics(&mut params)?;

        let RecvParams {
            ident, generics, ..
        } = params;

        let (impl_gens, ty_gens, where_clause) = generics.split_for_impl();

        let impl_block = parse_quote! {
            #[automatically_derived]
            impl #impl_gens #recv_trait_path for #ident #ty_gens #where_clause {
                #recv_from_def
            }
        };

        Ok(Self { impl_block })
    } 
    
    fn adjust_generics(params: &mut RecvParams) -> Result<()> {
        let mut bounds = Punctuated::new();
        bounds.push(
            syn::TraitBound {
                lifetimes: None,
                modifier: syn::TraitBoundModifier::None,
                paren_token: None,
                path: params.decode_trait_path(),
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

impl quote::ToTokens for RecvImpl {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.impl_block.to_tokens(tokens)
    }
}
