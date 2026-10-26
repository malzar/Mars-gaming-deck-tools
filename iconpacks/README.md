# Gestor de packs de iconos

Plugin para **OpenDeck** que gestiona packs de iconos de Stream Deck (`.streamDeckIconPack`) desde una ventana, sin tener que descomprimir nada a mano.

*English below.*

## Qué hace

- Una tecla que muestra cuántos packs tienes instalados (`Packs 3/5`). Al pulsarla abre el gestor.
- **Lista de packs** con casilla para instalar, papelera para desinstalar (borra los iconos) y una barra con la **paleta de colores** de cada pack.
- **Añadir pack**: eliges un `.streamDeckIconPack` o `.zip` y queda disponible.
- **Vigila Descargas**: cualquier pack que bajes aparece solo en la lista.
- **Abrir carpeta**: abre la carpeta de iconos para elegirlos desde OpenDeck (*Edit* → clic en la imagen).

## Carpetas

- Iconos instalados: `%USERPROFILE%\StreamDeck\Icons\<pack>\`
- Packs disponibles (copia de trabajo): `%USERPROFILE%\StreamDeck\IconPacks\`
- Registro: `%USERPROFILE%\StreamDeck\IconPacks\_registro.log`

## Requisitos

Windows 10/11, OpenDeck con Node.js 22 o superior y Microsoft Edge para la ventana del gestor.

## Instalación

Comprime la carpeta `com.malzar.iconpacks.sdPlugin` en un `.zip` (o descárgalo de *Releases*) y en OpenDeck: Plugins → *Install from file*.

## Licencia

MIT.

---

## English

OpenDeck plugin to manage Stream Deck icon packs from a window: install or uninstall packs with a checkbox, preview each pack's color palette, add packs from a file, and automatically pick up packs downloaded to your Downloads folder. Installed icons go to `%USERPROFILE%\StreamDeck\Icons`.
