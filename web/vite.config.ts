import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  // 相对路径，构建产物可直接部署到任意子目录（如 GitHub Pages）
  base: "./",
});
