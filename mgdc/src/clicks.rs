//! Click y doble click en teclas y ruedas.
//!
//! Cada pulsación espera un momento por si llega la segunda. Si el doble click
//! no hace nada, el click va al momento (no hace falta esperar).

use std::{collections::HashMap, sync::LazyLock, time::Duration};

use tokio::sync::Mutex;

/// Tiempo para que dos pulsaciones cuenten como doble click
pub const DOUBLE: Duration = Duration::from_millis(350);

/// Pulsaciones pendientes de cada tecla o rueda
pub struct Clicks {
    /// (pulsaciones sin decidir, número de la última)
    pending: LazyLock<Mutex<HashMap<String, (u8, u64)>>>,
}

impl Clicks {
    pub const fn new() -> Self {
        Self { pending: LazyLock::new(|| Mutex::new(HashMap::new())) }
    }

    /// Apunta una pulsación y espera a saber qué ha sido: `Some(false)` click,
    /// `Some(true)` doble click, `None` si esta pulsación ya la ha decidido otra
    /// (la segunda de un doble click). Con `double_does_something` a false el
    /// click se decide al momento.
    pub async fn press(&self, context: &str, double_does_something: bool) -> Option<bool> {
        let seq = {
            let mut map = self.pending.lock().await;
            let e = map.entry(context.to_string()).or_default();
            e.0 += 1;
            e.1 += 1;
            e.1
        };
        if !double_does_something {
            self.take(context).await;
            return Some(false);
        }
        tokio::time::sleep(DOUBLE).await;
        let n = match self.pending.lock().await.get(context) {
            // La última pulsación decide; o la primera, si ya hay dos
            Some(&(n, last)) if n > 0 && (last == seq || n >= 2) => n,
            _ => return None,
        };
        self.take(context).await;
        Some(n >= 2)
    }

    async fn take(&self, context: &str) {
        if let Some(e) = self.pending.lock().await.get_mut(context) {
            e.0 = 0;
        }
    }

    /// La tecla ya no se ve
    pub async fn forget(&self, context: &str) {
        self.pending.lock().await.remove(context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn click_y_doble() {
        static C: Clicks = Clicks::new();
        // Sin doble: al momento
        assert_eq!(C.press("a", false).await, Some(false));
        // Una sola pulsación: click al pasar el tiempo
        assert_eq!(C.press("a", true).await, Some(false));
        // Dos seguidas: un doble click y nada más (la segunda no hace un click)
        let first = tokio::spawn(async { C.press("b", true).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let second = tokio::spawn(async { C.press("b", true).await });
        assert_eq!(first.await.unwrap(), Some(true));
        assert_eq!(second.await.unwrap(), None);
        // Y después vuelve a contar desde cero
        assert_eq!(C.press("b", true).await, Some(false));
    }
}
