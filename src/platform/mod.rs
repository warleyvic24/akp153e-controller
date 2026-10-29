//! Integração com o sistema (Windows e Linux). Todo `unsafe` do projeto fica aqui, sempre com `// SAFETY:`.
//! `apps` e `audio` têm uma implementação por sistema, com a mesma interface; `shell` é um arquivo só,
//! com `cfg` nos pontos que diferem (janela, bandeja, início automático).
use std::process::Command;

#[cfg(windows)]
pub(crate) mod apps;
#[cfg(not(windows))]
#[path = "apps_linux.rs"]
pub(crate) mod apps;

#[cfg(windows)]
pub(crate) mod audio;
#[cfg(not(windows))]
#[path = "audio_linux.rs"]
pub(crate) mod audio;

mod audio_model;
pub(crate) mod omarchy;
pub(crate) mod shell;

/// No Windows, impede que um programa de console abra uma janela própria (o serviço não tem
/// janela). Nos outros sistemas não faz nada.
pub(crate) fn hide_console(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW)
    }
    #[cfg(not(windows))]
    {
        cmd
    }
}
