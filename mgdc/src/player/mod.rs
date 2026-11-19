//! Cómo se ve la tecla Reproducir/Pausa con lo que suena.
//!
//! - Sonando: título grande (en dos líneas si cabe; si no, una línea que se
//!   desliza), artista debajo y una barra abajo a elegir: nada, color plano,
//!   ondas, ecualizador (simulado o con el volumen real) o un
//!   GIF propio.
//! - En pausa: ▶ y el título en pequeño.
//! - Sin nada: solo ▶.
//!
//! En el MSD-ONE (teclas pequeñas y que aguantan pocos cambios) no hay GIF y
//!
//! - `key`: la tecla Reproducir/Pausa
//! - `dial`: la pantalla de las ruedas de volumen
//! - `eq`: el ecualizador (simulado o con el volumen real)
//! - `gif`: el GIF propio de la barra
//! - `text`: medir y recortar textos, y el deslizamiento de los que no caben

mod dial;
mod eq;
mod gif;
mod key;
mod music;
mod text;

pub use dial::{DialStyle, DialView, dial_needs_marquee, render_dial};
pub use eq::next_eq;
pub use gif::{gif_frame, save_gif};
pub use key::{moving, render};
pub use music::{IconPlace, MusicView, Pixels, music_moving, render_music_dial, render_music_skip};
pub use text::fit;

/// Barra de abajo mientras suena
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bar {
    None,
    Color,
    Waves,
    Eq,
    /// Ecualizador grande y suave, de fondo detrás del texto
    EqBackground,
    Gif,
}

impl Bar {
    pub fn from_id(id: &str) -> Bar {
        match id {
            "none" => Bar::None,
            "waves" => Bar::Waves,
            "eq" => Bar::Eq,
            "eqbg" => Bar::EqBackground,
            "gif" => Bar::Gif,
            _ => Bar::Color,
        }
    }

    /// Lo que se puede usar en un deck; en el MSD-ONE, todo menos el GIF
    pub fn for_deck(self, small: bool) -> Bar {
        if small && self == Bar::Gif { Bar::Color } else { self }
    }

    pub fn animated(self) -> bool {
        matches!(self, Bar::Waves | Bar::Eq | Bar::EqBackground | Bar::Gif)
    }

    /// Si dibuja un ecualizador (abajo o de fondo)
    pub fn uses_eq(self) -> bool {
        matches!(self, Bar::Eq | Bar::EqBackground)
    }
}

/// De dónde salen las barras del ecualizador
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EqMode {
    /// Bailan al azar mientras suena
    Simulated,
    /// Siguen el volumen real del programa que suena
    Level,
}

impl EqMode {
    pub fn from_id(id: &str) -> EqMode {
        match id {
            "sim" => EqMode::Simulated,
            _ => EqMode::Level,
        }
    }
}

/// Barras que se calculan; la tecla usa las 7 primeras, la rueda hasta 12
pub const EQ_BARS: usize = 12;
pub(super) const KEY_BARS: usize = 7;

/// Lo que cambia de un fotograma a otro
pub struct Frame<'a> {
    /// Segundos desde que empezó esta canción (deslizamiento y ondas)
    pub t: f32,
    /// Altura de cada barra del ecualizador, de 0 a 1
    pub eq: [f32; EQ_BARS],
    /// Fotograma del GIF propio, ya como imagen PNG en base64
    pub gif: Option<&'a str>,
}

impl Frame<'_> {
    pub fn still() -> Frame<'static> {
        Frame { t: 0.0, eq: [0.0; EQ_BARS], gif: None }
    }
}

/// Ajustes de la barra, tal como vienen de la tecla
pub struct Look<'a> {
    pub bar: Bar,
    pub color: &'a str,
    pub small: bool,
}
