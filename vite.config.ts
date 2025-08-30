// vite.config.ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(async () => ({
  plugins: [react()],
  
  // 添加多入口點配置
  build: {
    rollupOptions: {
      input: {
        main: './index.html',
        popup: './popup.html'
      }
    }
  },

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"]
    }
  }
}));