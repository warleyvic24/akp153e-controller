//! Mixer de volume do Windows (Core Audio): lista os apps com sessão de áudio e muda volume/mudo de cada um.
//! Cada chamada consulta o sistema de novo: as sessões mudam quando um app abre, fecha ou para de tocar.
use windows::core::{Interface, PWSTR};
use windows::Win32::Foundation::{CloseHandle, BOOL, MAX_PATH, S_OK};
use windows::Win32::Media::Audio::{
    eConsole, eRender, AudioSessionStateActive, AudioSessionStateExpired, IAudioSessionControl2,
    IAudioSessionEnumerator, IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

pub use super::audio_model::AudioSession;
use super::audio_model::{group, RawSession};

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

/// Programas com sessão de áudio no dispositivo padrão (tocando ou parados), um item por `.exe`,
/// em ordem estável de nome. Fica de fora a sessão de sons do sistema e as já encerradas.
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
    let mut raw = Vec::new();
    // SAFETY: `list` é uma interface válida do Core Audio.
    let count = unsafe { list.GetCount()? };
    for i in 0..count {
        // Uma sessão que falha (por exemplo, encerrada durante a leitura) não derruba as demais.
        match read_session(&list, i) {
            Ok(Some(session)) => raw.push(session),
            Ok(None) => {}
            Err(e) => log::debug!("mixer: sessão {i} ignorada: {e}"),
        }
    }
    Ok(group(raw))
}

fn session_list() -> AudioResult<IAudioSessionEnumerator> {
    let device = default_device()?;
    // SAFETY: interfaces do próprio Core Audio, contadas por referência.
    unsafe {
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
        manager.GetSessionEnumerator()
    }
}

/// `None` para sessões que não entram no mixer: encerradas, de sons do sistema ou sem processo.
fn read_session(list: &IAudioSessionEnumerator, index: i32) -> AudioResult<Option<RawSession>> {
    // SAFETY: `list` é válida e `index` está dentro de `GetCount`.
    unsafe {
        let control = list.GetSession(index)?.cast::<IAudioSessionControl2>()?;
        let state = control.GetState()?;
        // S_OK = é a sessão de sons do sistema; S_FALSE = é de um app (por isso não vale `is_ok()`).
        if state == AudioSessionStateExpired || control.IsSystemSoundsSession() == S_OK {
            return Ok(None);
        }
        let pid = control.GetProcessId()?;
        if pid == 0 {
            return Ok(None);
        }
        let level = control.cast::<ISimpleAudioVolume>()?;
        Ok(Some(RawSession {
            pid,
            exe_path: process_path(pid),
            volume: level.GetMasterVolume()?,
            muted: level.GetMute()?.as_bool(),
            active: state == AudioSessionStateActive,
        }))
    }
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

/// Aplica `change` ao volume de todas as sessões dos processos `pids`; sessões que falham são puladas.
fn with_processes_volume(
    pids: &[u32],
    change: impl Fn(&ISimpleAudioVolume) -> AudioResult<()>,
) -> AudioResult<()> {
    let list = session_list()?;
    // SAFETY: `list` é uma interface válida do Core Audio.
    let count = unsafe { list.GetCount()? };
    for i in 0..count {
        if let Ok(Some(level)) = volume_of_processes(&list, i, pids) {
            change(&level)?;
        }
    }
    Ok(())
}

/// O controle de volume da sessão `index`, se ela for de um dos processos `pids`.
fn volume_of_processes(
    list: &IAudioSessionEnumerator,
    index: i32,
    pids: &[u32],
) -> AudioResult<Option<ISimpleAudioVolume>> {
    // SAFETY: `index` está dentro de `GetCount`; as interfaces vêm do Core Audio.
    unsafe {
        let control = list.GetSession(index)?.cast::<IAudioSessionControl2>()?;
        if !pids.contains(&control.GetProcessId()?) {
            return Ok(None);
        }
        control.cast::<ISimpleAudioVolume>().map(Some)
    }
}

pub fn set_session_volume(pids: &[u32], volume: f32) -> AudioResult<()> {
    let volume = volume.clamp(0.0, 1.0);
    // SAFETY: `level` é uma interface válida e o contexto de evento nulo é permitido.
    with_processes_volume(pids, |level| unsafe {
        level.SetMasterVolume(volume, std::ptr::null())
    })
}

pub fn set_session_mute(pids: &[u32], muted: bool) -> AudioResult<()> {
    // SAFETY: igual a `set_session_volume`.
    with_processes_volume(pids, |level| unsafe {
        level.SetMute(BOOL::from(muted), std::ptr::null())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fala com o áudio real do PC: rode com `cargo test audio -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn lists_real_sessions() {
        for s in list_sessions() {
            println!("{s:?}");
        }
    }
}
