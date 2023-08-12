#[macro_use]
mod generated;

pub use generated::*;

use cstr::cstr;
use std::{
    arch::global_asm,
    cell::Cell,
    env,
    ffi::{CStr, CString, OsString},
    iter::once,
    os::unix::prelude::OsStringExt,
    path::Path,
    slice::from_raw_parts_mut,
    thread::LocalKey,
};

#[cfg(target_arch = "x86_64")]
macro_rules! tls_size {
    () => {
        0x100
    };
}

thread_local! {
    static INITIALIZED: Cell<bool> = Cell::new(false);
    static TABLE: Cell<[usize; num_funcs!()]> = Cell::new([0; num_funcs!()]);

    #[cfg(target_arch = "x86_64")]
    static GNU_TLS: Cell<[u8; tls_size!()]> = Cell::new([0; tls_size!()]);

    #[cfg(target_arch = "x86_64")]
    static BIONIC_TLS: Cell<[u8; tls_size!()]> = Cell::new([0; tls_size!()]);

    #[cfg(target_arch = "aarch64")]
    static GNU_TLS: Cell<usize> = Cell::new(0);

    #[cfg(target_arch = "aarch64")]
    static BIONIC_TLS: Cell<usize> = Cell::new(0);
}

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
    // Allocate space for table
    concat!("  sub rsp, 8 * ", num_funcs!()),
    "  mov rbx, rsp",
    "  ",
    // Save GNU thread-local storage
    concat!("  sub rsp, ", tls_size!()),
    "  mov rax, SYS_get_thread_area",
    "  mov rdi, rsp",
    "  syscall",
    "  test rax, rax",
    "  jnz abort",
    "  ",
    "  mov rdi, rbx", // Table
    "  mov rsi, rsp", // Stack pointer
    "  jmp {do_exec}",
    "  ",
    "return_pad:",
    // Restore stack pointer
    "  mov rsp, rdi",
    "  ",
    // Save Bionic thread-local storage
    "  mov rbx, rsp",
    concat!("  sub rbx, ", tls_size!()),
    "  mov rax, SYS_get_thread_area",
    "  mov rdi, rbx",
    "  syscall",
    "  test rax, rax",
    "  jnz abort",
    "  ",
    // Restore GNU thread-local storage
    "  mov r12, rsp",
    "  mov rax, SYS_set_thread_area",
    "  mov rdi, r12",
    "  syscall",
    "  test rax, rax",
    "  jnz abort",
    concat!("  add rsp, ", tls_size!()),
    "  ",
    // Write saved data to thread-local global variables
    "  mov rdi, r12", // GNU TLS
    "  mov rsi, rbx", // Bionic TLS
    "  mov rdx, rsp", // Table
    "  call {write_saved_data_to_tls}",
    concat!("  sub add, 8 * ", num_funcs!()),
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
    do_exec = sym do_exec,
    write_saved_data_to_tls = sym write_saved_data_to_tls,
);

#[cfg(target_arch = "aarch64")]
global_asm!(
    "  .section .text.load_thunks_asm,\"ax\",@progbits",
    "  .globl load_thunks_asm",
    "  .p2align 2",
    "  .type load_thunks_asm,@function",
    "load_thunks_asm:",
    // Save callee-saved registers
    "  stp x29, x30, [sp, #-16]!",
    "  stp x28, x27, [sp, #-16]!",
    "  stp x26, x25, [sp, #-16]!",
    "  stp x24, x23, [sp, #-16]!",
    "  stp x22, x21, [sp, #-16]!",
    "  stp x20, x19, [sp, #-16]!",
    "  ",
    // Allocate space for table
    concat!("  mov x0, (8 * ", num_funcs!(), " + 0xf) & ~0xf"),
    "  sub sp, sp, x0",
    "  mov x19, sp",
    "  ",
    // Save GNU thread-local storage
    "  mrs x0, tpidr_el0",
    "  str x0, [sp, #-16]!",
    "  ",
    "  mov x0, x19", // Table
    "  mov x1, sp", // Stack pointer
    "  b {do_exec}",
    "  ",
    "  .section .text.return_pad,\"ax\",@progbits",
    "  .globl return_pad",
    "  .p2align 2",
    "  .type return_pad,@function",
    "return_pad:",
    // Restore stack pointer
    "  mov sp, x0",
    "  ",
    // Save Bionic thread-local storage
    "  mrs x1, tpidr_el0",
    "  ",
    // Restore GNU thread-local storage
    "  ldr x0, [sp], #16",
    "  msr tpidr_el0, x0",
    "  ",
    "  mov x2, sp", // Table
    "  bl {write_saved_data_to_tls}",
    concat!("  mov x0, (8 * ", num_funcs!(), " + 0xf) & ~0xf"),
    "  add sp, sp, x0",
    "  ",
    // Restore callee-saved registers
    "  ldp x20, x19, [sp, #+16]!",
    "  ldp x22, x21, [sp, #+16]!",
    "  ldp x24, x23, [sp, #+16]!",
    "  ldp x26, x25, [sp, #+16]!",
    "  ldp x28, x27, [sp, #+16]!",
    "  ldp x29, x30, [sp, #+16]!",
    "  ",
    "  ret",
    do_exec = sym do_exec,
    write_saved_data_to_tls = sym write_saved_data_to_tls,
);

extern "C" {
    fn load_thunks_asm();
    fn return_pad();
}

#[cfg(target_arch = "x86_64")]
unsafe fn set_tls(tls: &'static LocalKey<Cell<[u8; tls_size!()]>>) {
    let tls = tls.with(|v| v.get());
    let result = libc::syscall(libc::SYS_set_thread_area, tls.as_ptr());
    assert!(result == 0);
}

#[cfg(target_arch = "aarch64")]
unsafe fn set_tls(tls: &'static LocalKey<Cell<usize>>) {
    let tls = tls.with(|v| v.get());
    std::arch::asm!(
        "msr tpidr_el0, {tls}",
        tls = in(reg) tls,
    );
}

unsafe fn set_gnu_tls() {
    set_tls(&GNU_TLS)
}

unsafe fn set_bionic_tls() {
    set_tls(&BIONIC_TLS)
}

#[cfg(target_arch = "x86_64")]
unsafe extern "C" fn write_saved_data_to_tls(
    gnu_tls: *const [u8; tls_size!()],
    bionic_tls: *const [u8; tls_size!()],
    table: *const [usize; num_funcs!()],
) {
    GNU_TLS.with(|v| v.set(*gnu_tls));
    BIONIC_TLS.with(|v| v.set(*bionic_tls));
    TABLE.with(|v| v.set(*table));
}

#[cfg(target_arch = "aarch64")]
unsafe extern "C" fn write_saved_data_to_tls(
    gnu_tls: usize,
    bionic_tls: usize,
    table: *const [usize; num_funcs!()],
) {
    GNU_TLS.with(|v| v.set(gnu_tls));
    BIONIC_TLS.with(|v| v.set(bionic_tls));
    TABLE.with(|v| v.set(*table));
}

unsafe fn dev_ext_props_deny(result: VkResult, len: *const u32, props: *mut VkExtensionProperties) {
    let blocked_names = [
        cstr!("VK_EXT_calibrated_timestamps"),
        cstr!("VK_EXT_extended_dynamic_state2"),
    ];
    let new_name = cstr!("libgnubionicpipe_disabled_feature").as_ptr();
    if result == VkResult_VK_SUCCESS && !props.is_null() {
        let len = *len as usize;
        let props = from_raw_parts_mut(props, len);
        for prop in props {
            let name_buf = &mut prop.extensionName as *mut _;
            let name = CStr::from_ptr(name_buf);
            if blocked_names.contains(&name) {
                libc::strcpy(name_buf, new_name);
            }
        }
    }
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

extern "C" fn do_exec(table_addr: usize, sp: usize) -> ! {
    let ld_path = "/system/bin/linker64";
    let bin_path = env::var("LIBGNUBIONICPIPE_BIONIC_END_PATH").unwrap();
    let data = include_bytes!("../build-inputs/bionic-end");
    std::fs::write(&bin_path, data).unwrap();
    let return_pad_addr = hex_fmt(return_pad as usize);
    let symbols = FUNC_NAMES.into_iter().map(str_to_c_string);
    // Calling the program directly instead of through the linker works,
    // except then the linker goes through a code path that queries /proc/self/exe,
    // which breaks the userland exec.
    let args = once(str_to_c_string(ld_path))
        .chain(once(string_to_c_string(bin_path)))
        .chain(once(return_pad_addr))
        .chain(once(hex_fmt(table_addr)))
        .chain(once(hex_fmt(sp)))
        .chain(once(str_to_c_string("libvulkan.so")))
        .chain(symbols)
        .collect::<Vec<CString>>();
    let env = env::vars_os()
        .filter(|(var, _)| var != "LD_PRELOAD" && var != "LD_LIBRARY_PATH")
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
    if !INITIALIZED.with(|v| v.get()) {
        load_thunks_asm();
        INITIALIZED.with(|v| v.set(true));
    }
}
