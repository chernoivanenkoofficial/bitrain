mod utils;
mod ast;
#[cfg(feature = "message")]
mod messages;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[cfg(feature = "message")]
#[proc_macro_derive(Encode, attributes(message))]
pub fn encode(input: TokenStream) -> TokenStream {    
    expand_derive(input, messages::encode)
}

#[cfg(feature = "message")]
#[proc_macro_derive(Decode, attributes(message))]
pub fn decode(input: TokenStream) -> TokenStream {   
    expand_derive(input, messages::decode)
}

#[cfg(feature = "message")]
#[proc_macro_derive(Standalone, attributes(message, standalone))]
pub fn standalone(input: TokenStream) -> TokenStream {   
    expand_derive(input, messages::standalone)
}

#[cfg(feature = "message")]
#[proc_macro_derive(Recv, attributes(message, standalone))]
pub fn recv(input: TokenStream) -> TokenStream {   
    expand_derive(input, messages::recv)
}


#[cfg(feature = "message")]
#[proc_macro_derive(Send, attributes(message, standalone))]
pub fn send(input: TokenStream) -> TokenStream {   
    expand_derive(input, messages::send)
}

fn expand_derive<F: FnOnce(DeriveInput) -> darling::Result<proc_macro2::TokenStream>>(input: TokenStream, implementor: F) -> TokenStream {
    implementor(parse_macro_input!(input))
        .unwrap_or_else(darling::Error::write_errors)
        .into()
}