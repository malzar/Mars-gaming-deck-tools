# MGDC — Mars Gaming Deck Controller

Un solo plugin de **OpenDeck** para controlar los dos decks de Mars Gaming y sus clones:

| Familia | Decks | Controles |
|---|---|---|
| AKP153 / Mirabox 293S | **Mars Gaming MSD-ONE**, Ajazz AKP153, Mirabox HSV293S, Soomfon, Womier D15… | 15 teclas + 3 iconos laterales |
| AKP05 / Mirabox N4 | **Mars Gaming MSD-PRO**, Ajazz AKP05, Mirabox N4, VSDInside N4, Redragon SS552… | 10 teclas, 4 ruedas, tira táctil |

Sustituye a los plugins *Ajazz AKP153 / Mirabox HSV293S* y *Ajazz AKP05 / Mirabox N4*. **No los tengas instalados a la vez**, porque se pelearían por los decks.

> Estado: **fase 1** (base unificada). Funciona igual que los dos plugins originales. Siguientes fases: panel de ajustes (brillo, apagado de pantalla, brillo de día y de noche según el sol), fondo del deck y animaciones de arranque y cierre.

## Instalación

1. En OpenDeck, desinstala *Ajazz AKP153 / Mirabox HSV293S* y *Ajazz AKP05 / Mirabox N4*.
2. Plugins → **Install from file** → `com.malzar.mgdc.sdPlugin.zip` (sin descomprimir).
3. Reinicia OpenDeck.

### Perfiles

- **MSD-ONE y resto de la familia AKP153:** mantienen sus perfiles, porque se usa el mismo identificador de dispositivo (`99-…`).
- **MSD-PRO y resto de la familia N4:** su identificador pasa de `n4-<serial>` a `99-<serial>`, así que OpenDeck lo verá como un deck nuevo. Para conservar los perfiles, con OpenDeck cerrado y **después de hacer una copia** de `%APPDATA%\opendeck\profiles`, renombra en esa carpeta lo que empiece por `n4-<serial>` a `99-<serial>`. Si algo no carga, restaura la copia y reconfigura el deck.

## Compilar

Requisitos: [Rust](https://rustup.rs) con las herramientas de compilación de Visual Studio (MSVC).

```powershell
cd mgdc
powershell -ExecutionPolicy Bypass -File .\package.ps1
```

Genera `dist\com.malzar.mgdc.sdPlugin.zip`.

## Configuración de LED y vibración (familia N4)

Se mantiene el `config.toml` del plugin de ambiso. MGDC lo busca en `%APPDATA%\mgdc\config.toml` y, si no existe, en `%APPDATA%\opendeck-akp05\config.toml`. En la fase 2 todo esto pasa al panel de ajustes.

## Créditos y licencia

GPL-3.0. Basado en:

- [opendeck-akp153](https://github.com/4ndv/opendeck-akp153), de Andrey Viktorov (4ndv), GPL-3.0.
- [opendeck-akp05](https://github.com/ambiso/opendeck-akp05), de ambiso, GPL-3.0.
- [mirajazz](https://github.com/4ndv/mirajazz) (versión de [ambiso](https://github.com/ambiso/mirajazz)).
