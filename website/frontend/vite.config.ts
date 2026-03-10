import { defineConfig } from 'vite';
import { resolve } from 'node:path';

export default defineConfig({
  publicDir: false,
  build: {
    // Where bundles end up (relative to project root)
    outDir: 'public/assets/bundles',
    emptyOutDir: true,

    // Emit manifest.json for backend lookup
    manifest: true,

    // We don't want Vite's default "assets" subfolder
    assetsDir: '',

    // Minify output
    minify: 'esbuild',

    rollupOptions: {
      input: {
        // 4 explicit entrypoints:
        'main.min': resolve(__dirname, 'bundles/main.js'),
        'main.min.css': resolve(__dirname, 'bundles/main.css'),
      },
      output: {
        // JS outputs
        entryFileNames: (chunk) => {
          // chunk.name will be "vendors.min" or "main.min"
          return `${chunk.name}-[hash].js`;
        },

        // CSS and other asset outputs
        assetFileNames: (assetInfo) => {
          // When CSS is emitted, it comes through here
          const name = assetInfo.name ?? '';

          // Our css entries are named "vendors.min.css" / "main.min.css"
          if (name === 'main.min.css') return `main-[hash].min.css`;

          // Fallback for anything else (fonts/images if they get pulled in)
          return `[name]-[hash][extname]`;
        },

        // Prevent Rollup from splitting into shared chunks
        // (keeps “vendors” and “main” isolated and predictable)
        manualChunks: undefined,
      },
    },
  },
});
