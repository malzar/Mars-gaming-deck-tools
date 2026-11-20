# Terceros: datos, código y servicios que usa Prisma

Prisma es software libre con licencia GPL-3.0 (ver `LICENSE`). Aquí está todo lo que no es de Prisma, con su licencia y lo que pide cada una.

## Datos

### Open-Meteo: el tiempo, la calidad del aire y la búsqueda de localidades

- **Qué se usa:** la [API de previsión](https://open-meteo.com/en/docs), la [API de calidad del aire](https://open-meteo.com/en/docs/air-quality-api) y la [API de búsqueda de localidades](https://open-meteo.com/en/docs/geocoding-api), sin cuenta ni clave.
- **Licencia de los datos:** [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Atribución: **"Weather data by [Open-Meteo.com](https://open-meteo.com/)"**. Prisma la muestra en la ventana de ajustes de «El tiempo» y en el panel, junto al buscador de localidades. Los datos se muestran resumidos (redondeados, con iconos y textos propios); no se modifican de otra forma. Open-Meteo no respalda ni patrocina Prisma.
- **Fuentes de Open-Meteo:** combina modelos de servicios meteorológicos nacionales (DWD, ECMWF, NOAA, Météo-France, MeteoSwiss, KNMI, Met Office y otros), cada uno con su licencia. La lista completa está en la [página de licencia de Open-Meteo](https://open-meteo.com/en/licence).
- **Calidad del aire y polen:** previsiones del Servicio de Vigilancia Atmosférica de Copernicus (CAMS), a través de Open-Meteo. Contiene información modificada del Copernicus Atmosphere Monitoring Service.
- **Localidades:** base de datos de [GeoNames](https://www.geonames.org/), [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), a través de Open-Meteo.
- **Condiciones de uso:** la API gratuita es para uso no comercial, con límites de 600 consultas por minuto, 5.000 por hora y 10.000 al día ([precios y límites](https://open-meteo.com/en/pricing)). Prisma pide los datos de cada sitio una vez cada 15 minutos y los comparte entre todas las teclas y ruedas, así que se queda muy por debajo.

## Webs a las que se enlaza

- **[Windy](https://www.windy.com/):** al pulsar «El tiempo» (si se elige esa opción) se abre Windy con la previsión del sitio. Solo es un enlace: Prisma no usa ni muestra datos de Windy. Windy tiene sus propias condiciones de uso y no tiene relación con Prisma.

## Código y fórmulas

- **[opendeck-akp153](https://github.com/4ndv/opendeck-akp153)**, de Andrey Viktorov (4ndv): GPL-3.0. Base del soporte del MSD-ONE.
- **[opendeck-akp05](https://github.com/ambiso/opendeck-akp05)**, de ambiso: GPL-3.0. Base del soporte del MSD-PRO.
- **[mirajazz](https://github.com/4ndv/mirajazz)** (versión de [ambiso](https://github.com/ambiso/mirajazz)): MPL-2.0. Comunicación por USB con los decks.
- **[openaction](https://github.com/OpenActionAPI/rust)**: MIT. Copia en `vendor/openaction` con un evento añadido («showSettingsInterface»).
- **[StreamDock-Device-SDK](https://github.com/MiraboxSpace/StreamDock-Device-SDK)**, de Mirabox: referencia del protocolo para la imagen de fondo.
- **Posición y fase de la luna, horas del sol:** fórmulas de [SunCalc](https://github.com/mourner/suncalc), reescritas en Rust en `src/astro.rs` y `src/sky/sun.rs`. Licencia BSD de 2 cláusulas:

  > Copyright (c) 2026, Volodymyr Agafonkin. All rights reserved.
  >
  > Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:
  >
  > 1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.
  > 2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.
  >
  > THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

- **Amanecer y puesta de sol:** la ecuación del amanecer estándar (`src/schedule.rs`), fórmulas astronómicas de dominio público.

- **Posición de los planetas:** elementos orbitales y método de Paul Schlyter, [«How to compute planetary positions»](https://stjarnhimlen.se/comp/ppcomp.html) (dominio público), en `src/sky/planets.rs`.
- **Estrellas:** [Yale Bright Star Catalog](http://tdc-www.harvard.edu/catalogs/bsc5.html), 5.ª ed. (Hoffleit y Warren, 1991; datos libres del NASA ADC), en `src/stars.rs`.

## Bibliotecas de Rust

Prisma se compila con estas bibliotecas (y las suyas). Todas tienen licencias compatibles con la GPL-3.0: sobre todo MIT y Apache-2.0, más alguna ISC, BSD, Zlib, Unicode-3.0 y MPL-2.0.

| Biblioteca | Para qué | Licencia |
|---|---|---|
| [tokio](https://github.com/tokio-rs/tokio), [tokio-util](https://github.com/tokio-rs/tokio) | Tareas en paralelo | MIT |
| [axum](https://github.com/tokio-rs/axum) | Servidor del panel | MIT |
| [serde](https://github.com/serde-rs/serde), [serde_json](https://github.com/serde-rs/json), [toml](https://github.com/toml-rs/toml) | Leer y guardar ajustes | MIT o Apache-2.0 |
| [image](https://github.com/image-rs/image) | Imágenes, GIF y fondos | MIT o Apache-2.0 |
| [chrono](https://github.com/chronotope/chrono) | Fechas y horas | MIT o Apache-2.0 |
| [ureq](https://github.com/algesten/ureq) | Consultas a Open-Meteo | MIT o Apache-2.0 |
| [rustls](https://github.com/rustls/rustls) | Conexiones seguras (https) | Apache-2.0, ISC o MIT |
| [ring](https://github.com/briansmith/ring) | Cifrado de rustls | Apache-2.0 e ISC |
| [webpki-roots](https://github.com/rustls/webpki-roots) | Certificados raíz de Mozilla | CDLA-Permissive-2.0 |
| [windows](https://github.com/microsoft/windows-rs) | Audio y teclas multimedia de Windows | MIT o Apache-2.0 |
| [base64](https://github.com/marshallpierce/rust-base64), [data-url](https://github.com/servo/rust-url), [dirs](https://github.com/soc/dirs-rs), [log](https://github.com/rust-lang/log), [env_logger](https://github.com/rust-cli/env_logger), [futures-lite](https://github.com/smol-rs/futures-lite) | Utilidades | MIT o Apache-2.0 |

La lista completa, con versiones, está en `Cargo.lock`.

## Otros

- **OpenDeck:** la rueda de perfiles usa la acción «Switch Profile» del paquete de inicio de OpenDeck (starterpack), que trae el propio OpenDeck. Prisma no la incluye.
- **Logo, iconos, fondo y sonidos de Prisma:** propios del proyecto. Los ocho sonidos de `assets/sounds` están generados por síntesis para Prisma; no vienen de ninguna biblioteca de sonidos. `cucaracha.wav` toca la melodía de «La Cucaracha», canción popular tradicional de dominio público.
- **Marcas:** Mars Gaming, Ajazz, Mirabox, Stream Deck, Windy y el resto de nombres son de sus dueños. Prisma no está afiliado a ninguno.
