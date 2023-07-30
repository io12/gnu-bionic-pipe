mod generated;

pub use generated::*;

use std::{
    arch::global_asm,
    env,
    ffi::{CString, OsString},
    iter::once,
    os::unix::prelude::OsStringExt,
    path::Path,
};

static mut INITIALIZED: bool = false;
static mut TABLE: [usize; FUNC_NAMES.len()] = [0; FUNC_NAMES.len()];
static mut SAVED_STACK_PTR: usize = 0;

#[cfg(target_arch = "x86_64")]
global_asm!(
    "load_thunks_asm:",
    // Save callee-saved registers
    "  push rbp",
    "  push r15",
    "  push r14",
    "  push r13",
    "  push r12",
    "  push rbx",
    "  ",
    // Save thread-local storage
    "  push fs",
    "  sub rsp, 0x100",
    "  mov rax, SYS_get_thread_area",
    "  mov rdi, rsp",
    "  syscall",
    "  test rax, rax",
    "  jnz abort",
    "  ",
    // Save stack pointer
    "  mov [rip + {}], rsp",
    "  ",
    "  jmp {}",
    sym SAVED_STACK_PTR,
    sym do_exec
);

#[cfg(target_arch = "x86_64")]
global_asm!(
    "return_pad:",
    // Restore stack pointer
    "  mov rsp, [rip + {}]",
    "  ",
    // Restore thread-local storage
    "  mov rax, SYS_set_thread_area",
    "  mov rdi, rsp",
    "  syscall",
    "  test rax, rax",
    "  jnz abort",
    "  add rsp, 0x100",
    "  pop fs",
    "  ",
    // Restore callee-saved registers
    "  pop rbx",
    "  pop r12",
    "  pop r13",
    "  pop r14",
    "  pop r15",
    "  pop rbp",
    "  ",
    "  ret",
    sym SAVED_STACK_PTR,
);

#[cfg(target_arch = "aarch64")]
global_asm!(
    "  .section .text.load_thunks_asm,\"ax\",@progbits",
    "  .globl load_thunks_asm",
    "  .p2align 2",
    "  .type load_thunks_asm,@function",
    "load_thunks_asm:",
    // Save callee-saved registers
    "  stp x29, x30, [sp, #-112]!",
    "  stp x28, x27, [sp, #16]",
    "  stp x26, x25, [sp, #32]",
    "  stp x24, x23, [sp, #48]",
    "  stp x22, x21, [sp, #64]",
    "  stp x20, x19, [sp, #80]",
    "  ",
    // Save thread-local storage
    "  mrs x0, tpidr_el0",
    "  mrs x1, fpcr",
    "  stp x0, x1, [sp, #96]",
    "  ",
    // Save stack pointer
    "  adrp x0, {sp}",
    "  add x0, x0, :lo12:{sp}",
    "  mov x1, sp",
    "  str x1, [x0]",
    "  ",
    "  b {do_exec}",
    sp = sym SAVED_STACK_PTR,
    do_exec = sym do_exec
);

#[cfg(target_arch = "aarch64")]
global_asm!(
    "  .section .text.return_pad,\"ax\",@progbits",
    "  .globl return_pad",
    "  .p2align 2",
    "  .type return_pad,@function",
    "return_pad:",
    // Restore stack pointer
    "  adrp x0, {sp}",
    "  add x0, x0, :lo12:{sp}",
    "  ldr x0, [x0]",
    "  mov sp, x0",
    "  ",
    // Restore thread-local storage
    "  ldp x0, x1, [sp, #96]",
    "  msr tpidr_el0, x0",
    "  msr fpcr, x1",
    "  ",
    // Restore callee-saved registers
    "  ldp x20, x19, [sp, #80]",
    "  ldp x22, x21, [sp, #64]",
    "  ldp x24, x23, [sp, #48]",
    "  ldp x26, x25, [sp, #32]",
    "  ldp x28, x27, [sp, #16]",
    "  ldp x29, x30, [sp], #112",
    "  ",
    "  ret",
    sp = sym SAVED_STACK_PTR,
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
    let ld_path = "/system/bin/linker64";
    let bin_path = env::var("LIBGNUBIONICPIPE_BIONIC_END_PATH").unwrap();
    let data = include_bytes!("../build-inputs/bionic-end");
    std::fs::write(&bin_path, data).unwrap();
    let return_pad_addr = hex_fmt(return_pad as usize);
    let table_addr = hex_fmt(unsafe { TABLE.as_mut_ptr() } as usize);
    let symbols = FUNC_NAMES.into_iter().map(str_to_c_string);
    // Calling the program directly instead of through the linker works,
    // except then the linker goes through a code path that queries /proc/self/exe,
    // which breaks the userland exec.
    let args = once(str_to_c_string(ld_path))
        .chain(once(string_to_c_string(bin_path)))
        .chain(once(return_pad_addr))
        .chain(once(table_addr))
        .chain(once(str_to_c_string("libvulkan.so")))
        .chain(symbols)
        .collect::<Vec<CString>>();
    let env = env::vars_os()
        .filter(|(var, _)| var != "LD_PRELOAD")
        .map(|(var, val)| {
            let s = [var, OsString::from("="), val]
                .into_iter()
                .collect::<OsString>()
                .into_vec();
            CString::new(s).unwrap()
        })
        .collect::<Vec<CString>>();
    userland_execve::exec(Path::new(ld_path), &args, &env)
}

unsafe fn init() {
    assert!(!INITIALIZED);
    load_thunks_asm();
    assert!(!INITIALIZED);
    INITIALIZED = true;
}
