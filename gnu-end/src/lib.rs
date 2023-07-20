mod generated;

pub use generated::*;

use std::{
    arch::global_asm,
    ffi::{CString, OsString},
    iter::once,
    os::unix::prelude::OsStringExt,
    path::Path,
};

static mut INITIALIZED: bool = false;
static mut TABLE: [usize; FUNC_NAMES.len()] = [0; FUNC_NAMES.len()];
static mut SAVED_STACK_PTR: usize = 0;
static mut SAVED_FRAME_PTR: usize = 0;
#[cfg(target_arg = "aarch64")]
static mut SAVED_LR: usize = 0;

#[cfg(target_arch = "x86_64")]
global_asm!(
    "load_thunks_asm:",
    "  mov [rip + {}], rsp",
    "  mov [rip + {}], rbp",
    "  jmp {}",
    sym SAVED_STACK_PTR,
    sym SAVED_FRAME_PTR,
    sym do_exec
);

#[cfg(target_arch = "x86_64")]
global_asm!(
    "return_pad:",
    "  mov rsp, [rip + {}]",
    "  mov rbp, [rip + {}]",
    "  ret",
    sym SAVED_STACK_PTR,
    sym SAVED_FRAME_PTR,
);

#[cfg(target_arch = "aarch64")]
global_asm!(
    "  .section .text.load_thunks_asm,\"ax\",@progbits",
    "  .globl load_thunks_asm",
    "  .p2align 2",
    "  .type load_thunks_asm,@function",
    "load_thunks_asm:",
    "  adrp x0, {sp}",
    "  add x0, x0, :lo12:{sp}",
    "  mov x1, sp",
    "  str x1, [x0]",
    "  ",
    "  adrp x0, {fp}",
    "  add x0, x0, :lo12:{fp}",
    "  str fp, [x0]",
    "  ",
    "  adrp x0, {lr}",
    "  add x0, x0, :lo12:{lr}",
    "  str lr, [x0]",
    "  ",
    "  b {do_exec}",
    sp = sym SAVED_STACK_PTR,
    fp = sym SAVED_FRAME_PTR,
    lr = sym SAVED_LR,
    do_exec = sym do_exec
);

#[cfg(target_arch = "aarch64")]
global_asm!(
    "  .section .text.return_pad,\"ax\",@progbits",
    "  .globl return_pad",
    "  .p2align 2",
    "  .type return_pad,@function",
    "return_pad:",
    "  adrp x0, {sp}",
    "  add x0, x0, :lo12:{sp}",
    "  ldr x0, [x0]",
    "  mov sp, x0",
    "  ",
    "  adrp x0, {fp}",
    "  add x0, x0, :lo12:{fp}",
    "  ldr fp, [x0]",
    "  ",
    "  adrp x0, {lr}",
    "  add x0, x0, :lo12:{lr}",
    "  ldr lr, [x0]",
    "  ",
    "  ret",
    sp = sym SAVED_STACK_PTR,
    fp = sym SAVED_FRAME_PTR,
    lr = sym SAVED_LR,
);

extern "C" {
    fn load_thunks_asm();
    fn return_pad();
}

fn string_to_c_string(s: String) -> CString {
    let mut bytes = s.into_bytes();
    bytes.push(0);
    CString::from_vec_with_nul(bytes).unwrap()
}

fn str_to_c_string(s: &str) -> CString {
    string_to_c_string(s.into())
}

fn hex_fmt(u: usize) -> CString {
    string_to_c_string(format!("{u:#x}"))
}

extern "C" fn do_exec() -> ! {
    let path = "/tmp/libgnubionicpipe-bionic-end";
    let data = include_bytes!("../build-inputs/bionic-end");
    std::fs::write(path, data).unwrap();
    let return_pad_addr = hex_fmt(return_pad as usize);
    let table_addr = hex_fmt(unsafe { TABLE.as_mut_ptr() } as usize);
    let symbols = FUNC_NAMES.into_iter().map(str_to_c_string);
    let args = once(str_to_c_string(path))
        .chain(once(return_pad_addr))
        .chain(once(table_addr))
        .chain(once(str_to_c_string("libvulkan.so")))
        .chain(symbols)
        .collect::<Vec<CString>>();
    let env = std::env::vars_os()
        .filter(|(var, _)| var != "LD_PRELOAD")
        .map(|(var, val)| {
            let s = [var, OsString::from("="), val]
                .into_iter()
                .collect::<OsString>()
                .into_vec();
            CString::new(s).unwrap()
        })
        .collect::<Vec<CString>>();
    userland_execve::exec(Path::new(path), &args, &env)
}

unsafe fn init() {
    assert!(!INITIALIZED);
    load_thunks_asm();
    assert!(!INITIALIZED);
    INITIALIZED = true;
}
