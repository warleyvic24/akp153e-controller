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

/// Um programa com sessão de áudio. Navegadores e apps Electron usam vários processos: todos os
/// processos do mesmo `.exe` formam um só item, e volume/mudo valem para todos eles.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioSession {
    /// Identifica o programa entre leituras: caminho do `.exe` em minúsculas (ou `pid:N` se ele não abriu).
    pub key: String,
    pub name: String,
    pub exe_path: Option<String>,
    pub pids: Vec<u32>,
    /// De 0.0 a 1.0; com vários processos, o maior.
    pub volume: f32,
    /// Só é `true` se todos os processos estão no mudo.
    pub muted: bool,
    /// Tocando agora (alguma sessão ativa); `false` = aberto mas parado.
    pub active: bool,
}

/// Uma sessão como o Windows a entrega, antes de juntar os processos do mesmo programa.
#[derive(Clone, Debug, PartialEq)]
struct RawSession {
    pid: u32,
    exe_path: Option<String>,
    volume: f32,
    muted: bool,
    active: bool,
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

/// Junta os processos do mesmo programa e ordena por nome (sem diferenciar maiúsculas) e chave.
fn group(raw: Vec<RawSession>) -> Vec<AudioSession> {
    let mut groups: Vec<AudioSession> = Vec::new();
    for s in raw {
        let key = s
            .exe_path
            .as_deref()
            .map_or_else(|| format!("pid:{}", s.pid), str::to_lowercase);
        match groups.iter_mut().find(|g| g.key == key) {
            Some(group) => {
                if !group.pids.contains(&s.pid) {
                    group.pids.push(s.pid);
                }
                group.volume = group.volume.max(s.volume);
                group.muted &= s.muted;
                group.active |= s.active;
            }
            None => groups.push(AudioSession {
                name: s
                    .exe_path
                    .as_deref()
                    .map_or_else(|| format!("PID {}", s.pid), display_name),
                key,
                exe_path: s.exe_path,
                pids: vec![s.pid],
                volume: s.volume,
                muted: s.muted,
                active: s.active,
            }),
        }
    }
    groups.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.key.cmp(&b.key))
    });
    groups
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

    fn raw(pid: u32, exe: Option<&str>, volume: f32, muted: bool, active: bool) -> RawSession {
        RawSession {
            pid,
            exe_path: exe.map(str::to_string),
            volume,
            muted,
            active,
        }
    }

    #[test]
    fn display_name_drops_folder_and_extension() {
        assert_eq!(display_name(r"C:\Apps\spotify.exe"), "Spotify");
        assert_eq!(display_name(r"C:\Apps\Discord.EXE"), "Discord");
    }

    #[test]
    fn display_name_of_empty_path_is_empty() {
        assert_eq!(display_name(""), "");
    }

    #[test]
    fn processes_of_the_same_program_become_one_item() {
        let chrome = r"C:\Chrome\chrome.exe";
        let grouped = group(vec![
            raw(10, Some(chrome), 0.4, true, false),
            raw(11, Some(chrome), 0.7, true, true),
            raw(10, Some(chrome), 0.4, true, false),
        ]);
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].pids, vec![10, 11]);
        assert_eq!(grouped[0].volume, 0.7);
        assert!(grouped[0].muted);
        assert!(grouped[0].active);
    }

    #[test]
    fn a_group_is_muted_only_if_every_process_is_muted() {
        let exe = r"C:\A\a.exe";
        let grouped = group(vec![
            raw(1, Some(exe), 0.5, true, true),
            raw(2, Some(exe), 0.5, false, true),
        ]);
        assert!(!grouped[0].muted);
    }

    #[test]
    fn the_path_match_ignores_case() {
        let grouped = group(vec![
            raw(1, Some(r"C:\A\App.exe"), 0.5, false, true),
            raw(2, Some(r"c:\a\app.EXE"), 0.5, false, true),
        ]);
        assert_eq!(grouped.len(), 1);
    }

    #[test]
    fn programs_are_ordered_by_name_whatever_the_windows_order() {
        let grouped = group(vec![
            raw(1, Some(r"C:\x\spotify.exe"), 0.5, false, true),
            raw(2, Some(r"C:\x\Discord.exe"), 0.5, false, true),
            raw(3, Some(r"C:\x\chrome.exe"), 0.5, false, true),
        ]);
        let names: Vec<_> = grouped.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["Chrome", "Discord", "Spotify"]);
    }

    #[test]
    fn a_process_whose_path_is_unreadable_stays_separate() {
        let grouped = group(vec![
            raw(7, None, 0.5, false, true),
            raw(8, None, 0.5, false, true),
        ]);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].name, "PID 7");
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
