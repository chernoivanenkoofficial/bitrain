mod decode;
mod encode;
mod recv;
mod send;
mod standalone;

pub use encode::encode;
pub use decode::decode;
pub use standalone::standalone;
pub use recv::recv;
pub use send::send;

static MOD_PATH: &str = "::bitrain_core::messages";

static ENCODE_TRAIT_NAME: &str = "Encode";
static DECODE_TRAIT_NAME: &str = "Decode";
static STANDALONE_TRAIT_NAME: &str = "Standalone";
static RECV_TRAIT_NAME: &str = "Recv";
static SEND_TRAIT_NAME: &str = "Send";

static CONTAINER_STRUCT_NAME: &str = "Container";

#[derive(Debug, darling::FromField)]
struct Field {
    ident: Option<syn::Ident>,
    ty: syn::Type
}

fn full_item_path(custom_mod_path: &Option<syn::Path>, mod_path: &str, trait_name: &str) -> syn::Path {
    let mut mod_path = custom_mod_path
        .to_owned()
        .unwrap_or(syn::parse_str(mod_path).unwrap());

    mod_path
        .segments
        .extend(syn::parse_str::<syn::PathSegment>(trait_name));

    mod_path
}