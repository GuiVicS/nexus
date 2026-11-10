//! Compatibilidade com glibc mais antiga (Raspberry Pi OS Bookworm = 2.36, Debian 12, Ubuntu 22.04).
//!
//! A biblioteca padrão do Rust referencia `pidfd_spawnp` e `pidfd_getpid` (glibc 2.39) como símbolos
//! *fracos*, mas o ligador marca a versão GLIBC_2.39 como obrigatória: o programa nem abre numa glibc
//! anterior. Elas só são usadas pelo recurso instável `CommandExt::create_pidfd`, que o Nexus não usa, e o
//! std trata `ENOSYS` voltando ao caminho normal (posix_spawn / fork+exec). Definir as duas aqui, no
//! executável, satisfaz a referência sem depender da glibc nova. Incluído só nos binários (`main.rs`):
//! numa biblioteca o ligador não puxaria este objeto para resolver uma referência fraca.
#![allow(unsafe_code)]

#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[unsafe(no_mangle)]
pub extern "C" fn pidfd_spawnp(
    _pidfd: *mut libc::c_int,
    _path: *const libc::c_char,
    _file_actions: *const libc::posix_spawn_file_actions_t,
    _attrp: *const libc::posix_spawnattr_t,
    _argv: *const *mut libc::c_char,
    _envp: *const *mut libc::c_char,
) -> libc::c_int {
    libc::ENOSYS
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[unsafe(no_mangle)]
pub extern "C" fn pidfd_getpid(_pidfd: libc::c_int) -> libc::pid_t {
    unsafe { *libc::__errno_location() = libc::ENOSYS };
    -1
}
