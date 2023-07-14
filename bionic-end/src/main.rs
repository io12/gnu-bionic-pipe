use libloading::os::unix::{Library, RTLD_NOW};
use std::{
    ffi::OsString,
    os::{raw::c_void, unix::prelude::OsStrExt},
};

#[derive(clap::Parser)]
struct Args {
    #[clap(value_parser = parse_int::parse::<usize>)]
    return_addr: usize,
    #[clap(value_parser = parse_int::parse::<usize>)]
    table_addr: usize,
    library: OsString,
    symbols: Vec<OsString>,
}

fn main() {
    let args = <Args as clap::Parser>::parse();

    // Load library
    let library = unsafe { Library::open(Some(args.library), RTLD_NOW) }.unwrap();

    // Write resolved symbols to table
    let table_ptr = args.table_addr as *mut usize;
    let table = unsafe { std::slice::from_raw_parts_mut(table_ptr, args.symbols.len()) };
    for (i, symbol) in args.symbols.into_iter().enumerate() {
        let symbol = symbol.as_bytes();
        let symbol = unsafe { library.get::<usize>(symbol) }.unwrap();
        let symbol = symbol.into_raw() as usize;
        table[i] = symbol;
    }

    // Jump to return address
    let return_ptr = args.return_addr as *const c_void;
    let return_fn: unsafe extern "C" fn() = unsafe { std::mem::transmute(return_ptr) };
    unsafe { return_fn() }
}
