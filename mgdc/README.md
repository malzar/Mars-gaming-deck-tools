# MGDC — Mars Gaming Deck Controller

Un solo plugin de **OpenDeck** para controlar los dos decks de Mars Gaming y sus clones:

| Familia | Decks | Controles |
|---|---|---|
| AKP153 / Mirabox 293S | **Mars Gaming MSD-ONE**, Ajazz AKP153, Mirabox HSV293S, Soomfon, Womier D15… | 15 teclas + 3 iconos laterales |
| AKP05 / Mirabox N4 | **Mars Gaming MSD-PRO**, Ajazz AKP05, Mirabox N4, VSDInside N4, Redragon SS552… | 10 teclas, 4 ruedas, tira táctil |

Sustituye a los plugins *Ajazz AKP153 / Mirabox HSV293S* y *Ajazz AKP05 / Mirabox N4*. **No los tengas instalados a la vez**, porque se pelearían por los decks.

> Estado: **fase 4** (animaciones e información).

## Instalación

1. En OpenDeck, desinstala *Ajazz AKP153 / Mirabox HSV293S* y *Ajazz AKP05 / Mirabox N4*.
2. Plugins → **Install from file** → `com.malzar.mgdc.sdPlugin.zip` (sin descomprimir).
3. Reinicia OpenDeck.

### Perfiles

- **MSD-ONE y resto de la familia AKP153:** mantienen sus perfiles, porque se usa el mismo identificador de dispositivo (`99-…`).
- **MSD-PRO y resto de la familia N4:** su identificador pasa de `n4-<serial>` a `99-<serial>`, así que OpenDeck lo verá como un deck nuevo. Para conservar los perfiles, con OpenDeck cerrado y **después de hacer una copia** de `%APPDATA%\opendeck\profiles`, renombra en esa carpeta lo que empiece por `n4-<serial>` a `99-<serial>`. Si algo no carga, restaura la copia y reconfigura el deck.

## Ajustes

En OpenDeck: **Plugins → MGDC → Ajustes del plugin**. Se abre una ventana con los decks conectados y, para cada uno:

- **Brillo**, que se aplica al momento mientras mueves el deslizador.
- **No apagar nunca la pantalla**, o apagarla tras X minutos sin usar el deck.
- **La pulsación que enciende la pantalla no ejecuta la acción**, para no disparar nada sin querer al despertar el deck.
- **Brillo de día y de noche**:
  - *Según el sol*: amanecer y puesta calculados a diario a partir de la ciudad o las coordenadas, sin internet. Se puede desplazar el cambio unos minutos antes o después.
  - *Horario fijo*: a qué hora empieza el día y a qué hora la noche.
  - Brillo de día, brillo de noche y una transición suave de X minutos.
- **Fondo del deck**: elige una imagen, ajústala (rellenar o encajar, con color de relleno) y pulsa *Aplicar al deck*. Se guarda en la memoria del propio deck, así que se ve también al arrancar y sin OpenDeck. Si un clon lo pinta girado o en espejo, hay una corrección de orientación.
- **Animaciones de arranque y de cierre**: ola, tecla a tecla, fundido o un GIF propio repartido por las teclas, con un color o una paleta. El cierre se ve al cerrar OpenDeck (al apagar el PC puede no dar tiempo).
- **Paletas**: 10 de serie (Arcoíris, Pastel, Aurora, Atardecer, Océano, Fuego, Synthwave, Hielo, Bosque y Neón) y las que crees tú.
- **Información**: modelo, VID:PID, número de serie, protocolo y pantalla; botones para identificar, reiniciar el deck y copiar los datos.
- **Identificar deck**: hace parpadear el deck para saber cuál es.

Los ajustes se guardan en `%APPDATA%\mgdc\settings.json` y una copia del último fondo de cada deck en `%APPDATA%\mgdc\backgrounds`.

Notas:
- "Apagar la pantalla" pone el brillo a 0, así al despertar no hace falta volver a enviar las imágenes.
- El brillo que pongas desde OpenDeck (por ejemplo con la acción *Device Brightness*) se guarda como brillo del deck. Si el horario de día y noche está activo, manda el horario.

## Compilar

Requisitos: [Rust](https://rustup.rs) con las herramientas de compilación de Visual Studio (MSVC).

```powershell
cd mgdc
powershell -ExecutionPolicy Bypass -File .\package.ps1
```

Genera `dist\com.malzar.mgdc.sdPlugin.zip`.

## Configuración de LED y vibración (familia N4)

Se mantiene el `config.toml` del plugin de ambiso. MGDC lo busca en `%APPDATA%\mgdc\config.toml` y, si no existe, en `%APPDATA%\opendeck-akp05\config.toml`. Más adelante pasará al panel de ajustes.

## Créditos y licencia

GPL-3.0. Basado en:

- [opendeck-akp153](https://github.com/4ndv/opendeck-akp153), de Andrey Viktorov (4ndv), GPL-3.0.
- [opendeck-akp05](https://github.com/ambiso/opendeck-akp05), de ambiso, GPL-3.0.
- [mirajazz](https://github.com/4ndv/mirajazz) (versión de [ambiso](https://github.com/ambiso/mirajazz)).
- Protocolo del fondo: [StreamDock-Device-SDK](https://github.com/MiraboxSpace/StreamDock-Device-SDK), de Mirabox.
