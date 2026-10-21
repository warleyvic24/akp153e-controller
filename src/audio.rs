//! Mixer de volume do Windows (Core Audio): lista os apps que estão tocando e muda volume/mudo de cada um.
//! Cada chamada consulta o sistema de novo: as sessões mudam quando um app abre, fecha ou para de tocar.
use windows::core::{Interface, PWSTR};
use windows::Win32::Foundation::{CloseHandle, BOOL, MAX_PATH, S_OK};
use windows::Win32::Media::Audio::{
    eConsole, eRender, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionEnumerator,
    IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// Um app com som ativo. `volume` vai de 0.0 a 1.0.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioSession {
    pub pid: u32,
    pub name: String,
    pub exe_path: Option<String>,
    pub volume: f32,
    pub muted: bool,
}

type AudioResult<T> = windows::core::Result<T>;

thread_local! {
    /// O COM é iniciado uma vez por thread e fica até ela terminar.
    static COM_READY: () = {
        // SAFETY: chamada sem ponteiros; se a thread já o iniciou de outro modo, o código de aviso é inofensivo.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    };
}

fn init_com() {
    COM_READY.with(|_| {});
}

fn default_device() -> AudioResult<IMMDevice> {
    init_com();
    // SAFETY: cria o enumerador COM com o CLSID oficial; os objetos devolvidos são contados por referência.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        enumerator.GetDefaultAudioEndpoint(eRender, eConsole)
    }
}

/// Apps com som ativo, sem a sessão de sons do sistema e sem repetir o mesmo processo.
pub fn list_sessions() -> Vec<AudioSession> {
    match try_list_sessions() {
        Ok(sessions) => sessions,
        Err(e) => {
            log::warn!("mixer: não foi possível listar os apps com som: {e}");
            Vec::new()
        }
    }
}

fn try_list_sessions() -> AudioResult<Vec<AudioSession>> {
    let list = session_list()?;
    let mut sessions: Vec<AudioSession> = Vec::new();
    // SAFETY: `list` é uma interface válida do Core Audio.
    let count = unsafe { list.GetCount()? };
    for i in 0..count {
        // Uma sessão que falha (por exemplo, encerrada durante a leitura) não derruba as demais.
        match read_session(&list, i) {
            Ok(Some(s)) if !sessions.iter().any(|known| known.pid == s.pid) => sessions.push(s),
            Ok(_) => {}
            Err(e) => log::debug!("mixer: sessão {i} ignorada: {e}"),
        }
    }
    Ok(sessions)
}

fn session_list() -> AudioResult<IAudioSessionEnumerator> {
    let device = default_device()?;
    // SAFETY: interfaces do próprio Core Audio, contadas por referência.
    unsafe {
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
        manager.GetSessionEnumerator()
    }
}

/// `None` para sessões que não entram no mixer: inativas, de sons do sistema ou sem processo.
fn read_session(list: &IAudioSessionEnumerator, index: i32) -> AudioResult<Option<AudioSession>> {
    // SAFETY: `list` é válida e `index` está dentro de `GetCount`.
    unsafe {
        let control = list.GetSession(index)?.cast::<IAudioSessionControl2>()?;
        // S_OK = é a sessão de sons do sistema; S_FALSE = é de um app (por isso não vale `is_ok()`).
        if control.GetState()? != AudioSessionStateActive || control.IsSystemSoundsSession() == S_OK
        {
            return Ok(None);
        }
        let pid = control.GetProcessId()?;
        if pid == 0 {
            return Ok(None);
        }
        let level = control.cast::<ISimpleAudioVolume>()?;
        let exe_path = process_path(pid);
        Ok(Some(AudioSession {
            pid,
            name: exe_path
                .as_deref()
                .map_or_else(|| format!("PID {pid}"), display_name),
            exe_path,
            volume: level.GetMasterVolume()?,
            muted: level.GetMute()?.as_bool(),
        }))
    }
}

/// Nome do programa sem a pasta e a extensão, com a primeira letra maiúscula.
pub fn display_name(path: &str) -> String {
    let stem = std::path::Path::new(path)
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let mut chars = stem.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn process_path(pid: u32) -> Option<String> {
    // SAFETY: o handle abre só para leitura de nome, é fechado ao fim e o buffer tem o tamanho informado.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; MAX_PATH as usize];
        let mut len = buffer.len() as u32;
        let queried = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(process);
        queried.ok()?;
        Some(String::from_utf16_lossy(&buffer[..len as usize]))
    }
}

/// Aplica `change` ao volume de todas as sessões do processo; sessões que falham são puladas.
fn with_process_volume(
    pid: u32,
    change: impl Fn(&ISimpleAudioVolume) -> AudioResult<()>,
) -> AudioResult<()> {
    let list = session_list()?;
    // SAFETY: `list` é uma interface válida do Core Audio.
    let count = unsafe { list.GetCount()? };
    for i in 0..count {
        if let Ok(Some(level)) = volume_of_process(&list, i, pid) {
            change(&level)?;
        }
    }
    Ok(())
}

/// O controle de volume da sessão `index`, se ela for do processo `pid`.
fn volume_of_process(
    list: &IAudioSessionEnumerator,
    index: i32,
    pid: u32,
) -> AudioResult<Option<ISimpleAudioVolume>> {
    // SAFETY: `index` está dentro de `GetCount`; as interfaces vêm do Core Audio.
    unsafe {
        let control = list.GetSession(index)?.cast::<IAudioSessionControl2>()?;
        if control.GetProcessId()? != pid {
            return Ok(None);
        }
        control.cast::<ISimpleAudioVolume>().map(Some)
    }
}

pub fn set_session_volume(pid: u32, volume: f32) -> AudioResult<()> {
    let volume = volume.clamp(0.0, 1.0);
    // SAFETY: `level` é uma interface válida e o contexto de evento nulo é permitido.
    with_process_volume(pid, |level| unsafe {
        level.SetMasterVolume(volume, std::ptr::null())
    })
}

pub fn set_session_mute(pid: u32, muted: bool) -> AudioResult<()> {
    // SAFETY: igual a `set_session_volume`.
    with_process_volume(pid, |level| unsafe {
        level.SetMute(BOOL::from(muted), std::ptr::null())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_drops_folder_and_extension() {
        assert_eq!(display_name(r"C:\Apps\spotify.exe"), "Spotify");
        assert_eq!(display_name(r"C:\Apps\Discord.EXE"), "Discord");
    }

    #[test]
    fn display_name_of_empty_path_is_empty() {
        assert_eq!(display_name(""), "");
    }

    /// Fala com o áudio real do PC: rode com `cargo test audio -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn lists_real_sessions() {
        for s in list_sessions() {
            println!("{s:?}");
        }
    }
}
