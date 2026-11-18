// Traduzioni dell'interfaccia. Il testo italiano è la chiave: `t('Gioca')`
// mostra "Gioca" in italiano e la voce del dizionario nelle altre lingue
// (con ripiego sull'italiano se manca). Le variabili si scrivono `{nome}`:
// `t('Installato in {istanza}', { istanza })`.
//
// I dizionari stanno in `i18n/<lingua>/*.ts`, un file per schermata, e
// vengono uniti qui: ognuno esporta `default` un Record<italiano, testo>.

export type Lang = 'it' | 'en';

export const LANGUAGES: [Lang, string][] = [
  ['it', 'Italiano'],
  ['en', 'English'],
];

const modules = import.meta.glob<{ default: Record<string, string> }>('./i18n/en/*.ts', { eager: true });
const EN: Record<string, string> = Object.assign({}, ...Object.values(modules).map((m) => m.default));

let current = $state<Lang>('it');

export function setLang(lang: Lang) {
  current = lang === 'en' ? 'en' : 'it';
  document.documentElement.lang = current;
}

export function lang(): Lang {
  return current;
}

/** Locale per date e numeri (`Intl`). */
export function locale(): string {
  return current === 'en' ? 'en-GB' : 'it-IT';
}

export function t(text: string, vars?: Record<string, string | number>): string {
  const out = current === 'en' ? (EN[text] ?? text) : text;
  return vars ? out.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m)) : out;
}
