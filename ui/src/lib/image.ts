// Scelta di immagini dal PC e preparazione per il backend (tutto nel
// browser: il backend riceve solo PNG piccoli già pronti).

import { t } from './i18n.svelte';

/** Apre la finestra di scelta file; `null` se l'utente annulla. */
export function pickFile(accept: string): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.onchange = () => resolve(input.files?.[0] ?? null);
    input.oncancel = () => resolve(null);
    input.click();
  });
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(t('Immagine non leggibile.')));
    img.src = src;
  });
}

function readAsDataUrl(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error(t('File non leggibile.')));
    reader.readAsDataURL(file);
  });
}

/** Ritaglia al centro e ridimensiona a un quadrato `size`×`size` PNG. */
export async function squarePng(file: File, size = 256): Promise<string> {
  // Come data: URL e non blob:, che la CSP dell'app non consente nelle immagini.
  const img = await loadImage(await readAsDataUrl(file));
  const side = Math.min(img.naturalWidth, img.naturalHeight);
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext('2d')!;
  ctx.imageSmoothingQuality = 'high';
  ctx.drawImage(img, (img.naturalWidth - side) / 2, (img.naturalHeight - side) / 2, side, side, 0, 0, size, size);
  return canvas.toDataURL('image/png');
}

/** Legge una skin: deve essere un PNG 64×64 (o 64×32, vecchio formato). */
export async function readSkin(file: File): Promise<string> {
  if (file.type && file.type !== 'image/png') throw new Error(t('La skin deve essere un file PNG.'));
  const data = await readAsDataUrl(file);
  const img = await loadImage(data);
  if (img.naturalWidth !== 64 || (img.naturalHeight !== 64 && img.naturalHeight !== 32)) {
    throw new Error(t('Una skin è un PNG 64×64 pixel: questa è {w}×{h}.', { w: img.naturalWidth, h: img.naturalHeight }));
  }
  return img.naturalHeight === 32 ? upgradeLegacy(img) : data;
}

/**
 * Vecchio formato 64×32 → 64×64, come fa il gioco: braccio e gamba
 * sinistri sono lo specchio di quelli destri. Così la skin funziona anche
 * dove sostituisce le texture moderne.
 */
function upgradeLegacy(img: HTMLImageElement): string {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 64;
  const ctx = canvas.getContext('2d')!;
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(img, 0, 0);
  const mirror = (sx: number, sy: number, dx: number, dy: number) => {
    ctx.save();
    ctx.translate(dx + 16, dy);
    ctx.scale(-1, 1);
    ctx.drawImage(img, sx, sy, 16, 16, 0, 0, 16, 16);
    ctx.restore();
  };
  mirror(0, 16, 16, 48); // gamba destra → sinistra
  mirror(40, 16, 32, 48); // braccio destro → sinistro
  return canvas.toDataURL('image/png');
}

/**
 * Vista frontale 2D di una skin (testa, corpo, braccia, gambe e strati
 * esterni), per le miniature. Nitida: niente sfumature tra i pixel.
 */
export async function skinFront(skin: string, slim: boolean, scale = 6): Promise<string> {
  const img = await loadImage(skin);
  const legacy = img.naturalHeight === 32;
  const arm = slim ? 3 : 4;
  const canvas = document.createElement('canvas');
  canvas.width = 16 * scale;
  canvas.height = 32 * scale;
  const ctx = canvas.getContext('2d')!;
  ctx.imageSmoothingEnabled = false;
  const part = (sx: number, sy: number, w: number, h: number, dx: number, dy: number, flip = false) => {
    ctx.save();
    if (flip) {
      ctx.translate((dx + w) * scale, dy * scale);
      ctx.scale(-1, 1);
      ctx.drawImage(img, sx, sy, w, h, 0, 0, w * scale, h * scale);
    } else {
      ctx.drawImage(img, sx, sy, w, h, dx * scale, dy * scale, w * scale, h * scale);
    }
    ctx.restore();
  };
  const ax = 4 - arm; // il braccio sottile è centrato nello spazio di 4
  // Base: testa, corpo, braccia, gambe (le parti sinistre nel vecchio
  // formato sono lo specchio di quelle destre).
  part(8, 8, 8, 8, 4, 0);
  part(20, 20, 8, 12, 4, 8);
  part(44, 20, arm, 12, ax, 8);
  if (legacy) part(44, 20, arm, 12, 12, 8, true);
  else part(36, 52, arm, 12, 12, 8);
  part(4, 20, 4, 12, 4, 20);
  if (legacy) part(4, 20, 4, 12, 8, 20, true);
  else part(20, 52, 4, 12, 8, 20);
  // Strati esterni (cappello, giacca, maniche, pantaloni).
  part(40, 8, 8, 8, 4, 0);
  if (!legacy) {
    part(20, 36, 8, 12, 4, 8);
    part(44, 36, arm, 12, ax, 8);
    part(52, 52, arm, 12, 12, 8);
    part(4, 36, 4, 12, 4, 20);
    part(4, 52, 4, 12, 8, 20);
  }
  return canvas.toDataURL('image/png');
}

/** Testa di una skin vista di fronte (viso + strato del cappello), nitida. */
export async function skinHead(skin: string, size = 64): Promise<string> {
  const img = await loadImage(skin);
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext('2d')!;
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(img, 8, 8, 8, 8, 0, 0, size, size);
  ctx.drawImage(img, 40, 8, 8, 8, 0, 0, size, size);
  return canvas.toDataURL('image/png');
}
