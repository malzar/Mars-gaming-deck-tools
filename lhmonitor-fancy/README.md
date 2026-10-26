# LHMonitor fancy

Plugin para **OpenDeck** y **Stream Deck** que muestra los datos de [Libre Hardware Monitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) en tus teclas, rotando entre elementos: núcleos de CPU, GPUs, discos, adaptadores de red, ventiladores y sensores de placa base.

*English below.*

## Qué hace

- **7 acciones**: CPU, GPU, Memoria, Almacenamiento, Red, Ventilación y Placa base.
- **Detección automática**: lee el `data.json` de LHM y reparte los sensores sin configurar nada a mano. Solo ofrece los datos que existen en tu equipo.
- **Rotación**: cada X segundos pasa al siguiente elemento (CPU #1 → CPU #2…).
- **Hasta 3 datos por tecla**, cada uno con su color y, cuando aplica, un umbral a partir del cual se pinta en rojo.
- **Vista combinada** con los datos a la vez: el primero en grande, con barra, y los otros dos debajo.
- **Pulsaciones**:
  - **1 clic**: cambia entre los datos marcados (y la vista combinada, si está incluida). Con un solo modo no hace nada.
  - **2 clics**: pausa o reanuda la rotación.
  - **3 clics**: salta al siguiente elemento.
- **Panel de configuración** en una ventana aparte, con vista previa real de la tecla.
- Oculta por defecto los ventiladores que nunca han girado y los adaptadores de red sin tráfico.
- Castellano e inglés según el idioma del sistema.

## Requisitos

- Windows 10/11.
- [Libre Hardware Monitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) con el servidor web activo: **Options → Remote Web Server → Run**. Ejecútalo como administrador para que lea todos los sensores.
- OpenDeck 2.x con Node.js 20 o superior, o Stream Deck 6.4 o superior.

## Instalación

- **OpenDeck**: Plugins → *Install from file* → elige `com.malzar.lhmmonitor.sdPlugin.zip` **sin descomprimir**. Reinicia OpenDeck.
- **Stream Deck**: doble clic en `com.malzar.lhmmonitor.streamDeckPlugin`.

## Uso

1. Arrastra una acción (por ejemplo **CPU**) a una tecla.
2. En los ajustes de la tecla, pulsa **Abrir configuración**.
3. Escribe la dirección de LHM (por ejemplo `http://localhost:8085` o `https://192.168.1.10:8085`) y pulsa **Probar**.
4. Elige datos, colores, umbrales, tiempos y elementos, y pulsa **Guardar**.

Cada tecla guarda su propia configuración, así que puedes tener una con los núcleos 1–8 y otra con el resto.

## Problemas

- **La tecla dice "Sin conexión"**: comprueba que el servidor web de LHM está en marcha y que la dirección del panel es la misma que la que muestra LHM (http o https, IP y puerto).
- **Registro**: `plugin.log` en la carpeta del plugin.

## Licencia

MIT. El texto de las teclas usa glifos de DejaVu Sans Condensed (licencia libre de Bitstream Vera / DejaVu).

---

## English

Shows Libre Hardware Monitor sensors on your OpenDeck / Stream Deck keys, rotating through CPU cores, GPUs, drives, network adapters, fans and motherboard sensors.

- 7 actions, with automatic sensor discovery from LHM's `data.json`.
- Up to 3 values per key, each with its own color and an optional red threshold, plus a combined view.
- **1 click**: next value · **2 clicks**: pause / resume rotation · **3 clicks**: next item.
- External settings window with a live key preview. Every key keeps its own settings.
- Requires LHM with *Options → Remote Web Server → Run* enabled. Works with `http` and `https` (self-signed certificates are accepted).
