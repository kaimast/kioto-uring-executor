extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::ItemFn;

fn token_stream_with_error(mut tokens: TokenStream, error: syn::Error) -> TokenStream {
    tokens.extend(TokenStream::from(error.into_compile_error()));
    tokens
}

#[proc_macro_attribute]
pub fn test(_args: TokenStream, item: TokenStream) -> TokenStream {
    let mut input: ItemFn = match syn::parse2(item.clone().into()) {
        Ok(it) => it,
        Err(e) => return token_stream_with_error(item, e),
    };

    if input.sig.asyncness.is_none() {
        panic!("the `async` keyword is missing from the test declaration");
    }

    input.sig.asyncness = None;

    let body = &input.block;
    let brace_token = input.block.brace_token;

    let header = quote! {
        #[::core::prelude::v1::test]
    };

    let tokio_expr = quote! {
        tokio_uring_executor::initialize();
        tokio_uring_executor::block_on(async {
            #body
        })
    };

    input.block = syn::parse2(quote! {
        {
            #tokio_expr
        }
    })
    .expect("Parsing failure");
    input.block.brace_token = brace_token;

    let result = quote! {
        #header
        #input
    };

    result.into()
}

#[proc_macro_attribute]
pub fn main(_args: TokenStream, item: TokenStream) -> TokenStream {
    let mut input: ItemFn = match syn::parse2(item.clone().into()) {
        Ok(it) => it,
        Err(e) => return token_stream_with_error(item, e),
    };

    if input.sig.asyncness.is_none() {
        panic!("the `async` keyword is missing from the main function declaration");
    }

    input.sig.asyncness = None;

    let body = &input.block;
    let brace_token = input.block.brace_token;

    let tokio_expr = quote! {
        tokio_uring_executor::initialize();
        tokio_uring_executor::block_on(async {
            #body
        })
    };

    input.block = syn::parse2(quote! {
        {
            #tokio_expr
        }
    })
    .expect("Parsing failure");
    input.block.brace_token = brace_token;

    quote!{
        #input
    }.into()
}
