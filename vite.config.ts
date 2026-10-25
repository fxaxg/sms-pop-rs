import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const root = (name: string) => new URL(`./${name}`, import.meta.url).pathname;

// Tauri 期望的开发端口；不要随意改
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Cargo 在工作区根目录生成 DLL，避免监听编译产物触发 Windows 文件锁错误。
      ignored: ["**/target/**"],
    },
  },
  build: {
    // 三个窗口入口：主设置窗 / toast 弹窗 / 光标候选条
    rollupOptions: {
      input: {
        main: root("index.html"),
        toast: root("toast.html"),
        caret: root("caret.html"),
      },
    },
  },
});
