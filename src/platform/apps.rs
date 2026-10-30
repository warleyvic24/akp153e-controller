//! Detecção de programas instalados (atalhos do Menu Iniciar) e extração do ícone do executável.
//! Usa o PowerShell do Windows; caminhos vão por variável de ambiente, nunca dentro do script.
use super::{icon_dir, icon_file_name};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const ICON_SIZE: u32 = 256;
const ENV_SOURCE: &str = "AKP_SRC";
const ENV_OUT: &str = "AKP_OUT";

const SCAN_SCRIPT: &str = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$sh = New-Object -ComObject WScript.Shell
$dirs = @("$env:ProgramData\Microsoft\Windows\Start Menu\Programs", "$env:APPDATA\Microsoft\Windows\Start Menu\Programs")
$found = Get-ChildItem $dirs -Recurse -Filter *.lnk -ErrorAction SilentlyContinue | ForEach-Object {
    $t = $sh.CreateShortcut($_.FullName).TargetPath
    if ($t -like '*.exe' -and $_.BaseName -notmatch 'uninstall|desinstal') { [pscustomobject]@{ name = $_.BaseName; target = $t; uwp = $false } }
}
$store = Get-StartApps | Where-Object { $_.AppID -match '!' } | ForEach-Object { [pscustomobject]@{ name = $_.Name; target = $_.AppID; uwp = $true } }
$found = @($found) + @($store) | Sort-Object name -Unique
ConvertTo-Json -InputObject @($found) -Compress
"#;

const ICON_SCRIPT: &str = r#"
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class IX{[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern uint PrivateExtractIcons(string f,int i,int cx,int cy,IntPtr[] h,uint[] id,uint n,uint fl);}'
$src = $env:AKP_SRC
$h = New-Object IntPtr[] 1
$id = New-Object uint32[] 1
$n = [IX]::PrivateExtractIcons($src, 0, __SIZE__, __SIZE__, $h, $id, 1, 0)
if ($n -ge 1 -and $h[0] -ne [IntPtr]::Zero) { $bmp = [System.Drawing.Icon]::FromHandle($h[0]).ToBitmap() }
else { $bmp = [System.Drawing.Icon]::ExtractAssociatedIcon($src).ToBitmap() }
$bmp.Save($env:AKP_OUT, [System.Drawing.Imaging.ImageFormat]::Png)
"#;

const UWP_ICON_SCRIPT: &str = r#"
$aumid = $env:AKP_SRC
$family, $appId = $aumid -split '!', 2
$pkg = Get-AppxPackage | Where-Object { $_.PackageFamilyName -eq $family } | Select-Object -First 1
if (-not $pkg) { exit 1 }
$manifest = Get-AppxPackageManifest -Package $pkg.PackageFullName
$apps = @($manifest.Package.Applications.Application)
$app = $apps | Where-Object { $_.Id -eq $appId } | Select-Object -First 1
if (-not $app) { $app = $apps[0] }
$ve = $app.VisualElements
$logo = $ve.Square44x44Logo
if (-not $logo) { $logo = $ve.Square150x150Logo }
if (-not $logo) { exit 1 }
$full = Join-Path $pkg.InstallLocation ($logo -replace '/', '\')
$dir = Split-Path $full
$stem = [System.IO.Path]::GetFileNameWithoutExtension($full)
$best = Get-ChildItem $dir -Filter "$stem*.png" -ErrorAction SilentlyContinue | Where-Object { $_.Name -notmatch 'altform-lightunplated|contrast' } | Sort-Object Length -Descending | Select-Object -First 1
if (-not $best) { exit 1 }
Copy-Item $best.FullName $env:AKP_OUT -Force
"#;

/// Programa detectado. `uwp`: app da Microsoft Store, identificado pelo AppUserModelID em `target`.
#[derive(Clone, Debug, PartialEq)]
pub struct InstalledApp {
    pub name: String,
    pub target: String,
    pub uwp: bool,
}

impl InstalledApp {
    pub fn exe(name: &str, path: &str) -> Self {
        Self {
            name: name.to_string(),
            target: path.to_string(),
            uwp: false,
        }
    }

    /// Programa e argumentos que a ação de abrir app deve usar.
    pub fn launch(&self) -> (String, Vec<String>) {
        if self.uwp {
            (
                "explorer.exe".to_string(),
                vec![format!("shell:AppsFolder\\{}", self.target)],
            )
        } else {
            (self.target.clone(), Vec::new())
        }
    }
}

fn powershell(script: &str) -> Command {
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Tempo máximo para extrair um ícone: um PowerShell travado não pode prender quem espera por ele
/// (o mixer lê os ícones numa thread que só roda uma por vez).
const ICON_TIMEOUT: Duration = Duration::from_secs(15);
const WAIT_STEP: Duration = Duration::from_millis(50);

/// Como `Command::output`, mas encerra o processo se ele passar de `limit`.
fn output_with_timeout(cmd: &mut Command, limit: Duration) -> std::io::Result<Output> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let start = Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() >= limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "o PowerShell demorou demais",
            ));
        }
        std::thread::sleep(WAIT_STEP);
    }
    child.wait_with_output()
}

/// Lista os programas do Menu Iniciar (nome e caminho do .exe), em ordem alfabética.
pub fn scan() -> Vec<InstalledApp> {
    match powershell(SCAN_SCRIPT).output() {
        Ok(out) if out.status.success() => parse_apps(&String::from_utf8_lossy(&out.stdout)),
        Ok(out) => {
            log::warn!(
                "busca de apps falhou: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            Vec::new()
        }
        Err(e) => {
            log::warn!("PowerShell indisponível: {e}");
            Vec::new()
        }
    }
}

pub fn parse_apps(json: &str) -> Vec<InstalledApp> {
    let Ok(value) =
        serde_json::from_str::<serde_json::Value>(json.trim().trim_start_matches('\u{feff}'))
    else {
        return Vec::new();
    };
    let items = match value {
        serde_json::Value::Array(a) => a,
        obj @ serde_json::Value::Object(_) => vec![obj],
        _ => Vec::new(),
    };
    items
        .iter()
        .filter_map(|v| {
            let uwp = v
                .get("uwp")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            Some(InstalledApp {
                name: v.get("name")?.as_str()?.to_string(),
                target: v.get("target")?.as_str()?.to_string(),
                uwp,
            })
        })
        .filter(|a| !a.name.is_empty() && !a.target.is_empty())
        .collect()
}

/// Extrai o ícone do executável para um PNG ao lado da configuração e devolve o caminho.
pub fn extract_icon(app: &InstalledApp) -> Option<String> {
    let target = app.target.as_str();
    if !app.uwp && !Path::new(target).is_file() {
        return None;
    }
    let dir = icon_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let out = dir.join(icon_file_name(target));
    if out.is_file() {
        return Some(out.display().to_string());
    }
    let script = if app.uwp {
        UWP_ICON_SCRIPT.to_string()
    } else {
        ICON_SCRIPT.replace("__SIZE__", &ICON_SIZE.to_string())
    };
    let status = output_with_timeout(
        powershell(&script)
            .env(ENV_SOURCE, target)
            .env(ENV_OUT, &out),
        ICON_TIMEOUT,
    );
    match status {
        Ok(o) if o.status.success() && out.is_file() => Some(out.display().to_string()),
        Ok(o) => {
            log::warn!(
                "ícone de {target} falhou: {}",
                String::from_utf8_lossy(&o.stderr)
            );
            None
        }
        Err(e) => {
            log::warn!("PowerShell indisponível: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_list_of_apps() {
        let apps = parse_apps(
            r#"[{"name":"Discord","target":"C:\\a\\Discord.exe"},{"name":"OBS","target":"C:\\b\\obs.exe"}]"#,
        );
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0], InstalledApp::exe("Discord", r"C:\a\Discord.exe"));
    }

    #[test]
    fn parses_an_empty_result() {
        assert!(parse_apps("[]").is_empty());
        assert!(parse_apps("").is_empty());
        assert!(parse_apps("lixo").is_empty());
    }

    #[test]
    fn store_apps_launch_through_explorer() {
        let apps = parse_apps(r#"{"name":"Spotify","target":"Spotify.Fam_x!App","uwp":true}"#);
        assert_eq!(apps.len(), 1);
        let (path, args) = apps[0].launch();
        assert_eq!(path, "explorer.exe");
        assert_eq!(
            args,
            vec![r"shell:AppsFolder\Spotify.Fam_x!App".to_string()]
        );
    }

    #[test]
    fn classic_apps_launch_directly() {
        let (path, args) = InstalledApp::exe("A", r"C:\a.exe").launch();
        assert_eq!((path.as_str(), args.len()), (r"C:\a.exe", 0));
    }

    #[test]
    fn skips_entries_without_name_or_target() {
        assert!(parse_apps(r#"[{"name":"","target":"x.exe"},{"name":"A"}]"#).is_empty());
    }

    #[test]
    fn icon_file_name_ignores_case_and_differs_per_program() {
        assert_eq!(icon_file_name(r"C:\A\x.exe"), icon_file_name(r"c:\a\X.EXE"));
        assert_ne!(icon_file_name(r"C:\A\x.exe"), icon_file_name(r"C:\A\y.exe"));
    }

    /// Usa o PowerShell de verdade (a pasta de ícones dos testes é temporária):
    /// `cargo test extracts_a_real_icon -- --ignored`.
    #[test]
    #[ignore]
    fn extracts_a_real_icon() {
        let app = InstalledApp::exe("Notepad", r"C:\Windows\System32\notepad.exe");
        let icon = extract_icon(&app).expect("ícone extraído");
        assert!(Path::new(&icon).is_file());
    }

    #[test]
    fn a_stuck_process_is_killed_after_the_timeout() {
        let mut cmd = powershell("Start-Sleep -Seconds 30");
        let started = Instant::now();
        let result = output_with_timeout(&mut cmd, Duration::from_millis(300));
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
