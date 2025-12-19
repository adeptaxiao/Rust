
use std::collections::BTreeMap;

#[macro_export]
macro_rules! btreemap {
    () => { $crate::BTreeMap::new() };
    ($($key:expr => $val:expr),+ $(,)?) => {{
        let mut map = $crate::BTreeMap::new();
        $(map.insert($key, $val);)+
        map
    }};
}

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Expr, Token, punctuated::Punctuated};

#[proc_macro]
pub fn btreemap_proc(input: TokenStream) -> TokenStream {
    let pairs = parse_macro_input!(input with Punctuated::<Expr, Token![,]>::parse_terminated);

    let mut inserts = Vec::new();
    for pair in pairs {
        if let Expr::Assign(expr_assign) = pair {
            let left = &expr_assign.left;
            let right = &expr_assign.right;
            inserts.push(quote! { map.insert(#left, #right); });
        }
    }

    let expanded = quote! {
        {
            let mut map = ::std::collections::BTreeMap::new();
            #(#inserts)*
            map
        }
    };

    TokenStream::from(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_btreemap_macro() {
        let map: BTreeMap<_, _> = btreemap!{
            "a" => 1,
            "b" => 2,
            "c" => 3,
        };
        assert_eq!(map["a"], 1);
        assert_eq!(map["b"], 2);
        assert_eq!(map["c"], 3);
    }

    #[test]
    fn test_btreemap_macro_empty() {
        let map: BTreeMap::<i32, i32> = btreemap!{};
        assert!(map.is_empty());
    }

}
