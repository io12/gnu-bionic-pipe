use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use std::{iter::once, path::Path};

fn make_thunk(index: usize, extern_c: &syn::ItemForeignMod) -> (String, TokenStream) {
    assert!(extern_c.attrs.is_empty());
    assert!(extern_c.unsafety.is_none());
    assert_eq!(extern_c.abi.name.as_ref().unwrap().value(), "C");
    assert_eq!(extern_c.items.len(), 1);
    let c_item = &extern_c.items[0];
    let c_fn = match c_item {
        syn::ForeignItem::Fn(c_fn) => c_fn,
        _ => panic!("item in `extern \"C\"` block is not an `fn`"),
    };
    assert!(matches!(c_fn.vis, syn::Visibility::Public(_)));
    assert!(c_fn.sig.constness.is_none());
    assert!(c_fn.sig.asyncness.is_none());
    assert!(c_fn.sig.unsafety.is_none());
    assert!(c_fn.sig.abi.is_none());
    assert!(c_fn.sig.variadic.is_none());
    let name = &c_fn.sig.ident;
    let args = &c_fn.sig.inputs;
    let return_type = &c_fn.sig.output;
    let arg_names = args.iter().map(|arg| match arg {
        syn::FnArg::Receiver(_) => panic!("unexpected `self` in `extern \"C\"` `fn`"),
        syn::FnArg::Typed(pat_type) => match &*pat_type.pat {
            syn::Pat::Ident(pat_ident) => {
                assert!(pat_ident.attrs.is_empty());
                assert!(pat_ident.by_ref.is_none());
                assert!(pat_ident.mutability.is_none());
                assert!(pat_ident.subpat.is_none());
                pat_ident.ident.to_token_stream()
            }
            _ => {
                panic!("bindgen generated an argument pattern more complex than just an identifier")
            }
        },
    });
    let arg_names = quote!(#(#arg_names),*);
    let name_string = name.to_token_stream().to_string();
    let thunk = quote! {
        #[no_mangle]
        pub unsafe extern "C" fn #name(#args) #return_type {
            if !crate::INITIALIZED {
                crate::init();
            }
            ::std::mem::transmute::<
                *mut ::std::os::raw::c_void,
                unsafe extern "C" fn(#args) #return_type,
            >(crate::TABLE[#index] as *mut ::std::os::raw::c_void)(#arg_names)
        }
    };
    (name_string, thunk)
}

fn main() {
    let bindings = bindgen::builder()
        .header("build-inputs/ndk-toolchain/sysroot/usr/include/GLES2/gl2.h")
        .clang_arg("-I./build-inputs/ndk-toolchain/lib64/clang/9.0.8/include")
        .clang_arg("-I./build-inputs/ndk-toolchain/sysroot/usr/include")
        .generate()
        .unwrap()
        .to_string();
    let bindings = syn::parse_file(&bindings).unwrap();
    assert!(bindings.shebang.is_none());
    assert!(bindings.attrs.is_empty());
    let type_defs = bindings.items.iter().map(|item| match item {
        syn::Item::ForeignMod(_) => TokenStream::new(),
        item => item.to_token_stream(),
    });
    let (func_names, thunk_defs): (Vec<String>, Vec<TokenStream>) = bindings
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::ForeignMod(extern_c) => Some(extern_c),
            _ => None,
        })
        .enumerate()
        .map(|(i, extern_c)| make_thunk(i, extern_c))
        .unzip();
    let num_funcs = func_names.len();
    let func_names_def = quote! {
        pub(crate) const FUNC_NAMES: [&str; #num_funcs] = [#(#func_names),*];
    };
    let generated_file = once(func_names_def)
        .chain(type_defs)
        .chain(thunk_defs)
        .collect::<TokenStream>()
        .to_string();
    let generated_file = syn::parse_file(&generated_file).unwrap();
    let generated_file = prettyplease::unparse(&generated_file);
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    std::fs::write(out_dir.join("generated.rs"), generated_file).unwrap()
}
