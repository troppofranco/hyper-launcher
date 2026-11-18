<script lang="ts">
  import type { SkinViewer } from 'skinview3d';
  import { prefersReducedMotion } from 'svelte/motion';

  // Anteprima 3D della skin: si gira trascinando, cammina sul posto. La
  // libreria 3D (three.js) si carica solo quando serve, non all'avvio.
  let { skin, slim, width = 300, height = 400 }: { skin: string | null; slim: boolean; width?: number; height?: number } = $props();

  let canvas: HTMLCanvasElement;
  let viewer = $state<SkinViewer | null>(null);

  $effect(() => {
    let disposed = false;
    let created: SkinViewer | null = null;
    import('skinview3d').then(({ SkinViewer, WalkingAnimation }) => {
      if (disposed) return;
      const v = new SkinViewer({ canvas, width, height });
      v.zoom = 0.85;
      v.fov = 50;
      v.camera.position.set(-18, 8, 42);
      v.controls.enableZoom = false;
      v.autoRotate = !prefersReducedMotion.current;
      v.autoRotateSpeed = 0.6;
      if (!prefersReducedMotion.current) {
        v.animation = new WalkingAnimation();
        v.animation.speed = 0.6;
      }
      created = v;
      viewer = v;
    });
    return () => {
      disposed = true;
      created?.dispose();
      viewer = null;
    };
  });

  $effect(() => {
    const s = skin;
    const model = slim ? 'slim' : 'default';
    if (!viewer) return;
    if (s) void viewer.loadSkin(s, { model });
    else viewer.resetSkin();
  });
</script>

<canvas bind:this={canvas} {width} {height} aria-label="Anteprima 3D della skin: trascina per girarla"></canvas>

<style>
  canvas { display: block; max-width: 100%; cursor: grab; }
  canvas:active { cursor: grabbing; }
</style>
