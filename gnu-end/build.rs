use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use std::{iter::once, path::Path};

type SynFuncArgs = syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>;

fn make_thunk_body_proc_addr(func_names: &[String]) -> TokenStream {
    let c_void = quote!(::std::os::raw::c_void);
    let match_arms = func_names
        .iter()
        .map(|name_string| {
            let name_bytes =
                syn::LitByteStr::new(name_string.as_bytes(), proc_macro2::Span::call_site());
            let name_ident = quote::format_ident!("{name_string}");
            quote! {
                #name_bytes => Some(
                    ::std::mem::transmute::<
                        *const #c_void,
                        unsafe extern "C" fn(),
                    >(#name_ident as *const #c_void)
                ),
            }
        })
        .collect::<TokenStream>();
    quote! {
        match ::std::ffi::CStr::from_ptr(pName).to_bytes() {
            #match_arms
            _ => None,
        }
    }
}

fn make_thunk_body(
    index: usize,
    name: &syn::Ident,
    args: &SynFuncArgs,
    return_type: &syn::ReturnType,
) -> TokenStream {
    let c_void = quote!(::std::os::raw::c_void);
    let not_loaded_message = format!("{name} not loaded");
    let arg_names = get_arg_names(args);
    quote! {
        if !crate::INITIALIZED {
            crate::init();
        }

        let void_ptr = crate::TABLE[#index] as *mut #c_void;
        assert!(!void_ptr.is_null(), #not_loaded_message);
        let func_ptr = ::std::mem::transmute::<
            *mut #c_void,
            unsafe extern "C" fn(#args) #return_type,
        >(void_ptr);

        crate::set_bionic_tls();
        let result = func_ptr(#arg_names);
        crate::set_gnu_tls();

        result
    }
}

fn get_arg_names(args: &SynFuncArgs) -> TokenStream {
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
    quote!(#(#arg_names),*)
}

fn make_thunk(index: usize, sig: &syn::Signature, func_names: &[String]) -> TokenStream {
    let name = &sig.ident;
    let args = &sig.inputs;
    let return_type = &sig.output;
    let proc_addr_body = make_thunk_body_proc_addr(func_names);
    let thunk_body = match name.to_string().as_str() {
        "vkGetInstanceProcAddr" => quote! { let _ = instance; #proc_addr_body },
        "vkGetDeviceProcAddr" => quote! { let _ = device; #proc_addr_body },
        _ => make_thunk_body(index, name, args, return_type),
    };
    quote! {
        #[no_mangle]
        pub unsafe extern "C" fn #name(#args) #return_type {
            #thunk_body
        }
    }
}

fn extern_c_to_signature(extern_c: &syn::ItemForeignMod) -> &syn::Signature {
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
    let sig = &c_fn.sig;
    assert!(sig.constness.is_none());
    assert!(sig.asyncness.is_none());
    assert!(sig.unsafety.is_none());
    assert!(sig.abi.is_none());
    assert!(sig.variadic.is_none());
    sig
}

fn get_function_signatures(bindings: &syn::File) -> Vec<&syn::Signature> {
    bindings
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::ForeignMod(extern_c) => Some(extern_c),
            _ => None,
        })
        .map(extern_c_to_signature)
        .collect()
}

fn main() {
    println!("cargo:rerun-if-changed=build-inputs");
    let bindings = bindgen::builder()
        .header("build-inputs/ndk-toolchain/sysroot/usr/include/vulkan/vulkan.h")
        .clang_arg("-I./build-inputs/ndk-toolchain/lib64/clang/9.0.8/include")
        .clang_arg("-I./build-inputs/ndk-toolchain/sysroot/usr/include")
        .generate()
        .unwrap()
        .to_string();
    let bindings = syn::parse_file(&bindings).unwrap();
    assert!(bindings.shebang.is_none());
    assert!(bindings.attrs.is_empty());
    let sigs = get_function_signatures(&bindings);
    let func_names = sigs
        .iter()
        .map(|sig| sig.ident.to_string())
        .collect::<Vec<String>>();
    let num_funcs = func_names.len();
    let func_names_def = quote! {
        pub(crate) const FUNC_NAMES: [&str; #num_funcs] = [#(#func_names),*];
    };
    let type_defs = bindings.items.iter().map(|item| match item {
        syn::Item::ForeignMod(_) | syn::Item::Impl(_) => TokenStream::new(),
        item => item.to_token_stream(),
    });
    let thunk_defs = sigs
        .iter()
        .enumerate()
        .map(|(i, sig)| make_thunk(i, sig, &func_names));
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
