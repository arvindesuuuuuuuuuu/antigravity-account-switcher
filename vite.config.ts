import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1421,
    strictPort: true,
    host: "127.0.0.1",
  },
  build: {
    target: "esnext",
    minify: "esbuild",
    sourcemap: false,
  },
});
