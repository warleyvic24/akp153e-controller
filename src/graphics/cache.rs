//! Caches de imagens vindas do disco (ícones, capas, imagem da tela lateral, GIFs).
//!
//! Sem eles, cada redesenho de tecla relia e decodificava o arquivo. Aqui cada entrada guarda o
//! carimbo do arquivo (data de modificação + tamanho): se o arquivo mudar, a próxima leitura vê o
//! carimbo diferente e decodifica de novo. As entradas saem pela ordem de uso (LRU) quando a soma
//! dos bytes decodificados passa do orçamento.
use image::{DynamicImage, RgbaImage};
use lru::LruCache;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::SystemTime;

/// Orçamento das imagens decodificadas (originais e variantes redimensionadas).
pub const IMAGE_BUDGET_BYTES: usize = 50 * 1024 * 1024;

/// Identidade de um arquivo no disco: muda quando o arquivo é regravado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl FileStamp {
    /// O carimbo atual de `path`; `None` se o arquivo não existe ou não é legível.
    pub fn of(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        meta.is_file().then(|| Self {
            modified: meta.modified().ok(),
            len: meta.len(),
        })
    }
}

struct Entry<V> {
    stamp: FileStamp,
    value: V,
    bytes: usize,
}

struct Inner<K: Hash + Eq, V> {
    map: LruCache<K, Entry<V>>,
    bytes: usize,
}

/// LRU limitado pela soma dos bytes das entradas. Leituras usam o lado de leitura do `RwLock`
/// (várias threads de desenho ao mesmo tempo); a promoção na ordem de uso é tentada sem esperar.
pub struct ByteLru<K: Hash + Eq, V> {
    inner: RwLock<Inner<K, V>>,
    budget: usize,
}

impl<K: Hash + Eq + Clone, V: Clone> ByteLru<K, V> {
    pub fn new(budget: usize) -> Self {
        Self {
            inner: RwLock::new(Inner {
                map: LruCache::unbounded(),
                bytes: 0,
            }),
            budget,
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner<K, V>> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner<K, V>> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// O valor guardado para `key`, se ainda for do arquivo com carimbo `stamp`.
    pub fn get(&self, key: &K, stamp: FileStamp) -> Option<V> {
        let value = {
            let inner = self.read();
            let entry = inner.map.peek(key)?;
            (entry.stamp == stamp).then(|| entry.value.clone())?
        };
        // Promoção sem bloquear: se outra thread estiver gravando, a ordem fica para a próxima.
        if let Ok(mut inner) = self.inner.try_write() {
            inner.map.promote(key);
        }
        Some(value)
    }

    /// Guarda `value` (que ocupa `bytes`) e tira as menos usadas até caber no orçamento.
    /// Um valor maior que o orçamento inteiro não é guardado.
    pub fn insert(&self, key: K, stamp: FileStamp, value: V, bytes: usize) {
        if bytes > self.budget {
            return;
        }
        let mut inner = self.write();
        if let Some(old) = inner.map.put(
            key,
            Entry {
                stamp,
                value,
                bytes,
            },
        ) {
            inner.bytes -= old.bytes;
        }
        inner.bytes += bytes;
        while inner.bytes > self.budget {
            match inner.map.pop_lru() {
                Some((_, old)) => inner.bytes -= old.bytes,
                None => break,
            }
        }
    }

    /// Esquece tudo o que veio de um arquivo (todas as chaves que `matches` aceitar).
    #[cfg(test)]
    pub fn forget(&self, matches: impl Fn(&K) -> bool) {
        let mut inner = self.write();
        let gone: Vec<K> = inner
            .map
            .iter()
            .filter(|(k, _)| matches(k))
            .map(|(k, _)| k.clone())
            .collect();
        for key in gone {
            if let Some(old) = inner.map.pop(&key) {
                inner.bytes -= old.bytes;
            }
        }
    }

    /// Bytes ocupados agora.
    #[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.read().bytes
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.read().map.len()
    }
}

/// O que guardar de uma imagem: a original decodificada ou uma variante derivada dela
/// (redimensionada, recortada...), identificada por um nome e um tamanho.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ImageKey {
    path: PathBuf,
    variant: Option<(&'static str, u32, u32)>,
}

#[derive(Clone)]
enum Cached {
    Original(Arc<DynamicImage>),
    Derived(Arc<RgbaImage>),
}

fn images() -> &'static ByteLru<ImageKey, Cached> {
    static CACHE: OnceLock<ByteLru<ImageKey, Cached>> = OnceLock::new();
    CACHE.get_or_init(|| ByteLru::new(IMAGE_BUDGET_BYTES))
}

fn dynamic_bytes(img: &DynamicImage) -> usize {
    img.as_bytes().len()
}

/// A imagem de `path` decodificada (do cache, se o arquivo não mudou).
pub fn image(path: impl AsRef<Path>) -> Option<Arc<DynamicImage>> {
    let path = path.as_ref();
    let stamp = FileStamp::of(path)?;
    let key = ImageKey {
        path: path.to_path_buf(),
        variant: None,
    };
    if let Some(Cached::Original(img)) = images().get(&key, stamp) {
        return Some(img);
    }
    let img = match image::open(path) {
        Ok(img) => Arc::new(img),
        Err(e) => {
            log::warn!("imagem ilegível {}: {e}", path.display());
            return None;
        }
    };
    let bytes = dynamic_bytes(&img);
    images().insert(key, stamp, Cached::Original(img.clone()), bytes);
    Some(img)
}

/// Uma variante de `path` (`kind` + tamanho a identificam) feita por `make` a partir da original.
/// Redimensionar uma capa grande a cada segundo custa mais que decodificá-la uma vez.
pub fn derived(
    path: impl AsRef<Path>,
    kind: &'static str,
    (w, h): (u32, u32),
    make: impl FnOnce(&DynamicImage) -> RgbaImage,
) -> Option<Arc<RgbaImage>> {
    let path = path.as_ref();
    let stamp = FileStamp::of(path)?;
    let key = ImageKey {
        path: path.to_path_buf(),
        variant: Some((kind, w, h)),
    };
    if let Some(Cached::Derived(img)) = images().get(&key, stamp) {
        return Some(img);
    }
    let original = image(path)?;
    let img = Arc::new(make(&original));
    let bytes = img.as_raw().len();
    images().insert(key, stamp, Cached::Derived(img.clone()), bytes);
    Some(img)
}

/// Esquece a imagem de `path` e as variantes dela.
#[cfg(test)]
pub fn forget_image(path: &Path) {
    images().forget(|k| k.path == path);
}

/// Bytes ocupados pelas imagens guardadas.
#[cfg(test)]
pub fn image_bytes() -> usize {
    images().bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("keyforge-cache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn stamp(len: u64) -> FileStamp {
        FileStamp {
            modified: None,
            len,
        }
    }

    #[test]
    fn the_least_recently_used_entry_goes_first() {
        let lru: ByteLru<u32, u32> = ByteLru::new(30);
        lru.insert(1, stamp(0), 10, 10);
        lru.insert(2, stamp(0), 20, 10);
        lru.insert(3, stamp(0), 30, 10);
        assert_eq!(lru.get(&1, stamp(0)), Some(10)); // 1 passa a ser o mais recente
        lru.insert(4, stamp(0), 40, 10);
        assert_eq!(lru.get(&2, stamp(0)), None, "2 era o menos usado");
        assert_eq!(lru.get(&1, stamp(0)), Some(10));
        assert_eq!(lru.bytes(), 30);
        assert_eq!(lru.len(), 3);
    }

    #[test]
    fn the_budget_is_never_exceeded() {
        let lru: ByteLru<u32, ()> = ByteLru::new(100);
        for i in 0..1000 {
            lru.insert(i, stamp(0), (), 7 + (i as usize % 13));
            assert!(lru.bytes() <= 100);
        }
        lru.insert(9999, stamp(0), (), 101);
        assert_eq!(lru.get(&9999, stamp(0)), None, "maior que o orçamento");
    }

    #[test]
    fn a_changed_stamp_is_a_miss() {
        let lru: ByteLru<u32, u32> = ByteLru::new(100);
        lru.insert(1, stamp(5), 1, 1);
        assert_eq!(lru.get(&1, stamp(6)), None);
        lru.insert(1, stamp(6), 2, 1);
        assert_eq!(lru.get(&1, stamp(6)), Some(2));
        assert_eq!(lru.bytes(), 1, "a entrada velha saiu da conta");
        lru.forget(|k| *k == 1);
        assert_eq!(lru.len(), 0);
        assert_eq!(lru.bytes(), 0);
    }

    #[test]
    fn a_rewritten_file_is_decoded_again() {
        let path = scratch("rewritten.png");
        RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]))
            .save(&path)
            .unwrap();
        let first = image(&path).unwrap();
        assert!(Arc::ptr_eq(&first, &image(&path).unwrap()), "veio do cache");
        // Tamanho diferente garante carimbo diferente mesmo com mtime de baixa resolução.
        RgbaImage::from_pixel(8, 4, image::Rgba([0, 0, 255, 255]))
            .save(&path)
            .unwrap();
        let second = image(&path).unwrap();
        assert_eq!(second.width(), 8);
        assert_eq!(second.to_rgba8().get_pixel(0, 0).0, [0, 0, 255, 255]);
        forget_image(&path);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn derived_variants_are_cached_per_size() {
        let path = scratch("derived.png");
        RgbaImage::from_pixel(64, 64, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .unwrap();
        let calls = std::cell::Cell::new(0);
        let make = |img: &DynamicImage| {
            calls.set(calls.get() + 1);
            img.thumbnail(16, 16).to_rgba8()
        };
        let a = derived(&path, "thumb", (16, 16), make).unwrap();
        let b = derived(&path, "thumb", (16, 16), make).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(calls.get(), 1);
        derived(&path, "thumb", (8, 8), make).unwrap();
        assert_eq!(calls.get(), 2, "outro tamanho é outra variante");
        forget_image(&path);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_missing_file_is_none() {
        assert!(image("/nao/existe/keyforge.png").is_none());
    }
}
