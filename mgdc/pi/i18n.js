// Idiomas de las ventanas de ajustes de Prisma.
//
// Los textos están en ../locales/<idioma>.json, sección «pi». En el HTML cada
// texto lleva su clave (data-i18n; data-i18n-ph y data-i18n-title para el
// placeholder y el título) y en
// el código se usa T('clave'). El HTML trae el español escrito, así que si no
// se pudiera leer el fichero de idioma se vería en español.
(function () {
  let dict = {};

  // Lee un fichero de idioma (síncrono: las ventanas pintan justo después)
  function read(code) {
    try {
      const x = new XMLHttpRequest();
      x.open('GET', `../locales/${code}.json`, false);
      x.send();
      if (x.status === 200 || (x.status === 0 && x.responseText)) return JSON.parse(x.responseText).pi || {};
    } catch (e) { /* sin fichero: se queda lo que haya */ }
    return {};
  }

  // Lo que falte en un idioma sale del siguiente: el pedido, su base («zh» de
  // «zh_CN»), inglés y español
  function load(code) {
    code = code || navigator.language || 'es';
    const chain = [...new Set([code.replace('-', '_'), code.split(/[-_]/)[0], 'en', 'es'])];
    dict = {};
    chain.reverse().forEach((c) => Object.assign(dict, read(c)));
    document.documentElement.lang = code.split(/[-_]/)[0];
  }

  window.T = (key) => dict[key] || key;

  function apply() {
    document.querySelectorAll('[data-i18n]').forEach((e) => { if (dict[e.dataset.i18n]) e.textContent = dict[e.dataset.i18n]; });
    document.querySelectorAll('[data-i18n-ph]').forEach((e) => { if (dict[e.dataset.i18nPh]) e.placeholder = dict[e.dataset.i18nPh]; });
    document.querySelectorAll('[data-i18n-title]').forEach((e) => { if (dict[e.dataset.i18nTitle]) e.title = dict[e.dataset.i18nTitle]; });
  }

  // Lo llama cada ventana al conectarse, con el «info» de OpenDeck (trae su idioma)
  window.prismaLang = (info) => {
    let code = '';
    try { code = (typeof info === 'string' ? JSON.parse(info) : info).application.language; } catch (e) { /* sin idioma: el del sistema */ }
    load(code);
    apply();
  };
})();
