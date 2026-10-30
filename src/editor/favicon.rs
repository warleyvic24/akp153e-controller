//! Ícone do site (favicon) nas teclas "Abrir site": em vez do globo genérico, a tecla mostra o ícone
//! que o próprio site publica. Busca direto no site que o usuário configurou (nenhum serviço de
//! terceiros fica sabendo quais sites ele usa), em segundo plano e só depois que a URL parou de
//! mudar, para não buscar a cada letra digitada.
use super::picker::{ButtonSlot, IconDone};
use crate::config::schema::{AppConfig, ButtonAction};
use crate::editor::library::URL_OPENER;
use reqwest::Url;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// A URL precisa ficar parada este tempo antes da busca.
const SETTLE: Duration = Duration::from_millis(1500);
const TIMEOUT: Duration = Duration::from_secs(6);
/// Limite do que se lê do site: a página só importa até o `<head>`, e um ícone é pequeno.
const MAX_PAGE_BYTES: u64 = 512 * 1024;
const MAX_ICON_BYTES: u64 = 1024 * 1024;
/// Quantos candidatos tentar antes de desistir.
const MAX_TRIES: usize = 4;
/// Ícone padrão das teclas "Abrir site": só ele é trocado; um ícone escolhido pelo usuário fica.
pub const SITE_ICON: &str = "globe";

/// Site aberto por uma ação "Abrir site" (o abridor do sistema com uma URL http/https).
pub fn site_url(action: &ButtonAction) -> Option<Url> {
    let ButtonAction::LaunchApp { path, args } = action else {
        return None;
    };
    if path != URL_OPENER || args.len() != 1 {
        return None;
    }
    let url = Url::parse(args[0].trim()).ok()?;
    let host = url.host_str()?;
    // "https://" sozinho (a ação recém-criada) ou um host sem ponto ainda está sendo digitado.
    let typed = host.contains('.') || host == "localhost";
    (matches!(url.scheme(), "http" | "https") && typed).then_some(url)
}

/// Links de ícone do HTML, do melhor para o pior: o maior declarado (o `apple-touch-icon` tem 180
/// px), SVG antes de PNG pequeno; por último os locais convencionais da raiz.
pub fn icon_links(html: &str, page: &Url) -> Vec<Url> {
    let mut found: Vec<(u32, Url)> = tags(html, "link")
        .filter_map(|tag| {
            let rel = attr(tag, "rel")?.to_lowercase();
            // `mask-icon` é um SVG de uma cor só (o Safari o pinta): fica de fora.
            let usable = |r: &str| r == "icon" || r.starts_with("apple-touch-icon");
            if !rel.split_whitespace().any(usable) {
                return None;
            }
            let url = page.join(&attr(tag, "href")?).ok()?;
            Some((link_score(tag, &rel, &url), url))
        })
        .collect();
    found.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    let mut urls: Vec<Url> = found.into_iter().map(|(_, u)| u).collect();
    // Locais convencionais, mesmo sem `<link>`: o ícone de 180 px que muitos sites publicam sem
    // declarar, e por fim o `/favicon.ico` (pequeno, mas quase todo site tem).
    for root in ["/apple-touch-icon.png", "/favicon.ico"] {
        if let Ok(root) = page.join(root) {
            if !urls.contains(&root) {
                urls.push(root);
            }
        }
    }
    urls
}

fn link_score(tag: &str, rel: &str, url: &Url) -> u32 {
    const APPLE_TOUCH_PX: u32 = 180;
    const SVG_SCORE: u32 = 1000;
    if url.path().to_lowercase().ends_with(".svg")
        || attr(tag, "type").is_some_and(|t| t.contains("svg"))
    {
        return SVG_SCORE;
    }
    let declared = attr(tag, "sizes").and_then(|s| {
        s.split_whitespace()
            .filter_map(|size| size.split(['x', 'X']).next()?.parse::<u32>().ok())
            .max()
    });
    match declared {
        Some(px) => px,
        None if rel.contains("apple-touch") => APPLE_TOUCH_PX,
        None => 16,
    }
}

/// Cada `<name ...>` do HTML (sem o `<name` e o `>`), sem diferenciar maiúsculas.
fn tags<'a>(html: &'a str, name: &str) -> impl Iterator<Item = &'a str> {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{name}");
    let mut starts = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find(&open) {
        let at = from + i + open.len();
        from = at;
        // `<linkx` não é `<link`.
        if lower[at..].starts_with(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>') {
            starts.push(at);
        }
    }
    starts
        .into_iter()
        .filter_map(move |at| html[at..].find('>').map(|end| &html[at..at + end]))
}

/// Valor do atributo `name` na tag (com aspas duplas, simples ou sem aspas).
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = tag[at + name.len()..].trim_start();
        let (true, Some(value)) = (before_ok, rest.strip_prefix('=')) else {
            continue;
        };
        let value = value.trim_start();
        let parsed = match value.chars().next()? {
            q @ ('"' | '\'') => value[1..].split(q).next()?,
            _ => value.split(|c: char| c.is_ascii_whitespace()).next()?,
        };
        return Some(decode_entities(parsed));
    }
    None
}

/// As entidades que aparecem em URLs de atributo (`&amp;` principalmente).
fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Busca o ícone do site e guarda como PNG; devolve o caminho. Bloqueia: rode fora da interface.
pub fn fetch(site: &Url) -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(concat!("akp153e/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;
    let get = |url: &Url, limit: u64| -> Option<(Url, Vec<u8>)> {
        let resp = client
            .get(url.clone())
            .send()
            .ok()?
            .error_for_status()
            .ok()?;
        let final_url = resp.url().clone();
        let mut bytes = Vec::new();
        resp.take(limit).read_to_end(&mut bytes).ok()?;
        Some((final_url, bytes))
    };
    // Sem a página (fora do ar, bloqueada) ainda dá para tentar o /favicon.ico.
    let candidates = match get(site, MAX_PAGE_BYTES) {
        Some((page, html)) => icon_links(&String::from_utf8_lossy(&html), &page),
        None => icon_links("", site),
    };
    let key = site.host_str().unwrap_or_default().to_string();
    candidates
        .iter()
        .take(MAX_TRIES)
        .find_map(|url| crate::platform::save_icon(&key, &get(url, MAX_ICON_BYTES)?.1))
}

/// Quais teclas "Abrir site" já ganharam a busca, e desde quando cada URL está parada.
#[derive(Default)]
pub struct Favicons {
    seen: HashMap<(ButtonSlot, String), Instant>,
    requested: HashSet<String>,
}

impl Favicons {
    /// Começa a busca das teclas "Abrir site" da página ativa que ainda mostram o globo. O ícone
    /// chega por `tx` e só é aplicado se a tecla ainda abrir o mesmo site (ver `IconDone`).
    pub fn poll(&mut self, cfg: &AppConfig, tx: &Sender<IconDone>) {
        let (profile, page) = (
            cfg.active_profile,
            cfg.profiles[cfg.active_profile].active_page,
        );
        let now = Instant::now();
        let mut current = HashSet::new();
        for (key, b) in cfg.page().buttons.iter().enumerate() {
            let waiting =
                b.appearance.icon_path.is_none() && b.appearance.icon.as_deref() == Some(SITE_ICON);
            let Some(url) = site_url(&b.action).filter(|_| waiting) else {
                continue;
            };
            let id = ((profile, page, key), url.to_string());
            let since = *self.seen.entry(id.clone()).or_insert(now);
            current.insert(id.clone());
            if now.duration_since(since) < SETTLE || !self.requested.insert(id.1.clone()) {
                continue;
            }
            let (tx, action, at) = (tx.clone(), b.action.clone(), id.0);
            std::thread::spawn(move || match fetch(&url) {
                Some(icon) => {
                    let _ = tx.send(IconDone { at, action, icon });
                }
                None => log::info!(
                    "site {} sem ícone utilizável",
                    url.host_str().unwrap_or("?")
                ),
            });
        }
        // Esquece URLs que sumiram (digitação no meio do caminho), para não crescer sem fim.
        self.seen.retain(|id, _| current.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Url {
        Url::parse("https://exemplo.com/blog/post").unwrap()
    }

    #[test]
    fn only_open_site_actions_with_a_real_url_count() {
        let site = |url: &str| ButtonAction::LaunchApp {
            path: URL_OPENER.into(),
            args: vec![url.into()],
        };
        assert!(site_url(&site("https://github.com")).is_some());
        assert!(site_url(&site("https://")).is_none(), "ação recém-criada");
        assert!(site_url(&site("https://git")).is_none(), "ainda digitando");
        assert!(site_url(&site("ftp://a.com")).is_none());
        let app = ButtonAction::LaunchApp {
            path: "firefox".into(),
            args: vec!["https://a.com".into()],
        };
        assert!(site_url(&app).is_none());
    }

    #[test]
    fn the_biggest_declared_icon_comes_first_and_favicon_ico_last() {
        let html = r#"<html><head>
            <link rel="icon" href="/fav-16.png" sizes="16x16">
            <LINK REL="apple-touch-icon" HREF="https://cdn.exemplo.com/touch.png">
            <link rel="stylesheet" href="/x.css">
            <link rel='icon' type='image/png' sizes='32x32 192x192' href='icons/big.png?v=1&amp;b=2'>
            <link rel="mask-icon" href="/mask.svg">
            <link rel="icon" type="image/svg+xml" href="/logo">
        </head></html>"#;
        let links: Vec<String> = icon_links(html, &page())
            .iter()
            .map(Url::to_string)
            .collect();
        assert_eq!(
            links,
            vec![
                "https://exemplo.com/logo",
                "https://exemplo.com/blog/icons/big.png?v=1&b=2",
                "https://cdn.exemplo.com/touch.png",
                "https://exemplo.com/fav-16.png",
                "https://exemplo.com/apple-touch-icon.png",
                "https://exemplo.com/favicon.ico",
            ]
        );
    }

    /// Busca na internet de verdade: `cargo test fetches_real_favicons -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fetches_real_favicons() {
        for site in [
            "https://github.com",
            "https://www.youtube.com",
            "https://pt.wikipedia.org",
            "https://www.google.com",
        ] {
            println!("{site} -> {:?}", fetch(&Url::parse(site).unwrap()));
        }
    }

    #[test]
    fn a_page_without_links_still_tries_favicon_ico() {
        let links: Vec<String> = icon_links("<html></html>", &page())
            .iter()
            .map(Url::to_string)
            .collect();
        assert_eq!(
            links,
            vec![
                "https://exemplo.com/apple-touch-icon.png",
                "https://exemplo.com/favicon.ico"
            ]
        );
    }

    #[test]
    fn link_like_tags_and_data_attributes_are_not_confused() {
        let html =
            r#"<linkage rel="icon" href="/no.png"><link data-rel="x" rel="icon" href="/yes.png">"#;
        let links = icon_links(html, &page());
        assert_eq!(links[0].path(), "/yes.png");
        assert_eq!(links.len(), 3);
    }
}
