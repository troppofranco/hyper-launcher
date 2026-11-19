// Righe della console di un server: prefisso "[hh:mm:ss LIVELLO]:" separato
// dal testo, e codici colore § di Minecraft trasformati in pezzi colorati.

export interface Piece { text: string; color: string | null; bold: boolean }
export interface ParsedLine { prefix: string; level: 'info' | 'warn' | 'error'; pieces: Piece[] }

// Paper: "[21:14:02 INFO]: …"; vanilla: "[21:14:02] [Server thread/INFO]: …".
const PREFIX = /^(\[[^\]]*\](?: \[[^\]]*\])?:)\s?/;
const CODE = /§([0-9a-fk-or])/gi;

/** Toglie i codici colore § (per copiare il testo). */
export function plain(text: string): string {
  return text.replace(CODE, '');
}

export function parseLine(text: string): ParsedLine {
  const m = PREFIX.exec(text);
  const prefix = m ? m[1] : '';
  const head = prefix.toUpperCase();
  const level = /\bERROR\b|\bSEVERE\b|\/ERROR\b/.test(head) ? 'error' : /\bWARN(ING)?\b|\/WARN\b/.test(head) ? 'warn' : 'info';
  const body = m ? text.slice(m[0].length) : text;

  const pieces: Piece[] = [];
  let color: string | null = null;
  let bold = false;
  let last = 0;
  const push = (s: string) => { if (s) pieces.push({ text: s, color, bold }); };
  for (const c of body.matchAll(CODE)) {
    push(body.slice(last, c.index));
    last = c.index + c[0].length;
    const code = c[1].toLowerCase();
    if (/[0-9a-f]/.test(code)) {
      // Nel gioco un colore azzera anche il grassetto.
      color = code;
      bold = false;
    } else if (code === 'l') bold = true;
    else if (code === 'r') {
      color = null;
      bold = false;
    }
    // Gli altri (corsivo, sottolineato, offuscato…) si ignorano.
  }
  push(body.slice(last));
  return { prefix, level, pieces };
}
