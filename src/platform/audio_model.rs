//! Dados do mixer comuns a todos os sistemas: a sessão de áudio, o agrupamento dos processos do mesmo
//! programa e o nome exibido. Cada sistema (udio.rs no Windows, udio_linux.rs no Linux) preenche
//! RawSession; o resto é igual.

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
pub(crate) struct RawSession {
    pub(crate) pid: u32,
    pub(crate) exe_path: Option<String>,
    pub(crate) volume: f32,
    pub(crate) muted: bool,
    pub(crate) active: bool,
}

/// Junta os processos do mesmo programa e ordena por nome (sem diferenciar maiúsculas) e chave.
pub(crate) fn group(raw: Vec<RawSession>) -> Vec<AudioSession> {
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

/// Nome do programa sem a pasta e a extensão, com a primeira letra maiúscula.
pub fn display_name(path: &str) -> String {
    // Aceita `\` e `/`: o nome vem do sistema que gerou o caminho, não do que está rodando.
    let file = path.rsplit(['/', '\\']).next().unwrap_or_default();
    let stem = match file.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => file,
    };
    let mut chars = stem.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
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
}
