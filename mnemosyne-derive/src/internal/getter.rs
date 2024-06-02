use super::{AttributeArgs, DIRECTIVE, STATE};
use syn::{meta::ParseNestedMeta, Attribute, Lit};

pub fn get_str_lit(meta: &ParseNestedMeta) -> Result<String, syn::Error> {
    let expr: syn::Expr = meta.value()?.parse()?;

    if let syn::Expr::Lit(syn::ExprLit {
        lit: Lit::Str(lit_str),
        ..
    }) = expr
    {
        Ok(lit_str.value())
    } else {
        Err(syn::Error::new_spanned(
            expr,
            "Only string literals are supported",
        ))
    }
}

pub fn get_inner_attribute(attrs: &Vec<Attribute>, att: &str) -> Result<AttributeArgs, syn::Error> {
    let mut state = None;
    let mut directive = None;

    for attr in attrs {
        if !attr.path().is_ident(att) {
            break;
        }

        if let Err(err) = attr.parse_nested_meta(|meta| {
            if meta.path == STATE {
                let res = get_str_lit(&meta)?;
                state = Some(res);
                Ok(())
            } else if meta.path == DIRECTIVE {
                let res = get_str_lit(&meta)?;
                directive = Some(res);
                Ok(())
            } else {
                Err(syn::Error::new_spanned(
                    meta.path,
                    "Only `state` and `directive` attributes are supported",
                ))
            }
        }) {
            return Err(err).map_err(|err| syn::Error::new_spanned(attr, err));
        }
    }

    Ok(AttributeArgs { directive, state })
}
