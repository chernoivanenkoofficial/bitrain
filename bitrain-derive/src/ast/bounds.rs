use syn::{*, punctuated::Punctuated};
use std::borrow::ToOwned;

pub trait Bind<T, B> {
    fn bind(&mut self, target: &T, bounds: impl IntoIterator<Item = B>);
    fn bind_all(&mut self, bounds: impl IntoIterator<Item = B>);
}

impl Bind<Ident, TypeParamBound> for Punctuated<GenericParam, Token![,]> {
    fn bind(&mut self, target: &syn::Ident, bounds: impl IntoIterator<Item = TypeParamBound>) {
        self.iter_mut().filter_map(|param| match param {
            GenericParam::Type(type_param) => Some(type_param),
            _ => None,
        })
        .find(|type_param| target == &type_param.ident)
        .map(|type_param| type_param.bounds.extend(bounds));
    }

    fn bind_all(&mut self, bounds: impl IntoIterator<Item = TypeParamBound>) {
        let bounds = bounds.into_iter().collect::<Vec<_>>();
        
        self.iter_mut().filter_map(|param| match param {
            syn::GenericParam::Type(type_param) => Some(type_param),
            _ => None,
        })
        .for_each(|type_param| type_param.bounds.extend(bounds.iter().map(ToOwned::to_owned)))      
    }
} 

impl<'a> Bind<syn::Ident, &'a syn::TypeParamBound> for Punctuated<GenericParam, Token![,]> {
    fn bind(&mut self, target: &syn::Ident, bounds: impl IntoIterator<Item = &'a syn::TypeParamBound>) {
        self.bind(target, bounds.into_iter().map(ToOwned::to_owned))
    }

    fn bind_all(&mut self, bounds: impl IntoIterator<Item = &'a syn::TypeParamBound>) {
        let bounds = bounds.into_iter().collect::<Vec<_>>();

        self.iter_mut().filter_map(|param| match param {
            syn::GenericParam::Type(type_param) => Some(type_param),
            _ => None,
        })
        .for_each(|type_param| type_param.bounds.extend(bounds.iter().map(|&b| b.to_owned())))
    }
}

impl Bind<syn::Ident,syn::TraitBound> for Punctuated<GenericParam, Token![,]> {
    fn bind(&mut self, target: &syn::Ident, bounds: impl IntoIterator<Item = syn::TraitBound>) {
        self.bind(target, bounds.into_iter().map(TypeParamBound::Trait))
    }

    fn bind_all(&mut self, bounds: impl IntoIterator<Item = syn::TraitBound>) {
        self.bind_all(bounds.into_iter().map(TypeParamBound::Trait))
    }
} 

impl<'a> Bind<syn::Ident, &'a syn::TraitBound> for Punctuated<GenericParam, Token![,]> {
    fn bind(&mut self, target: &syn::Ident, bounds: impl IntoIterator<Item = &'a syn::TraitBound>) {
        self.bind(target, bounds.into_iter().map(|bound| TypeParamBound::Trait(bound.to_owned())))
    }

    fn bind_all(&mut self, bounds: impl IntoIterator<Item = &'a syn::TraitBound>) {
        self.bind_all(bounds.into_iter().map(|bound| TypeParamBound::Trait(bound.to_owned())))
    }
} 

pub trait Find<'a, T: 'a> {
    fn find(&'a self, filter: impl FnMut(&T) -> bool) -> Vec<&'a T>;
    fn find_all(&'a self) -> Vec<&'a T> {
        Find::find(self, |_| true)
    }
}

pub trait FindMut<'a, T: 'a> {
    fn find_mut(&'a mut self, filter: impl FnMut(&T) -> bool) -> Vec<&'a mut T>;
    fn find_all_mut(&'a mut self) -> Vec<&'a mut T> {
        FindMut::find_mut(self, |_| true)
    }
}

impl Find<'_, syn::TypeParam> for syn::Generics {
    fn find(&self, mut filter: impl FnMut(&syn::TypeParam) -> bool) -> Vec<&syn::TypeParam> {
        self.params
            .iter()
            .filter_map(|param| match param {
                syn::GenericParam::Type(ty_param) => Some(ty_param),
                _ => None,
            })
            .filter(|param| filter(*param))
            .collect()
    }
}

impl FindMut<'_, syn::TypeParam> for syn::Generics {
    fn find_mut(&mut self, mut filter: impl FnMut(&syn::TypeParam) -> bool) -> Vec<&mut syn::TypeParam> {
        self.params
            .iter_mut()
            .filter_map(|param| match param {
                syn::GenericParam::Type(ty_param) => Some(ty_param),
                _ => None,
            })
            .filter(|param| filter(*param))
            .collect()
    }
}