//! Trava única para todo `Mutex` do programa.
//!
//! Um pânico com a trava presa "envenena" o `Mutex`. Aqui os dados guardados são sempre
//! consistentes entre instruções (estado de tela, fila, instância do teclado virtual), então o
//! certo é seguir usando o valor em vez de derrubar outra thread com um segundo pânico.
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Trava `mutex`, recuperando o valor se uma thread anterior entrou em pânico com ele travado.
pub fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poisoned_mutex_still_yields_its_value() {
        let shared = std::sync::Arc::new(Mutex::new(41));
        let poisoner = shared.clone();
        let crashed = std::thread::spawn(move || {
            let _guard = lock(&poisoner);
            panic!("pânico de propósito com a trava presa");
        })
        .join();
        assert!(crashed.is_err());
        assert!(shared.is_poisoned());
        *lock(&shared) += 1;
        assert_eq!(*lock(&shared), 42);
    }

    /// Nenhum `Mutex` trava fora deste helper: `.lock().unwrap()` derrubaria a thread num
    /// envenenamento, e o `unwrap_or_else` repetido divergia de lugar para lugar.
    #[test]
    fn every_mutex_goes_through_the_helper() {
        let mut wrong = Vec::new();
        for (file, src) in crate::test_support::sources_without_tests() {
            if file == "sync.rs" {
                continue;
            }
            let flat: String = src.split_whitespace().collect();
            if flat.contains(".lock().unwrap") || flat.contains(".lock().expect") {
                wrong.push(file);
            }
        }
        assert!(wrong.is_empty(), "use crate::sync::lock em: {wrong:?}");
    }
}
