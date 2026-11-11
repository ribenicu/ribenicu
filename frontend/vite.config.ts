import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// MPA：保留原静态站 URL（index.html / archive.html / post.html?p=slug / admin.html）
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://127.0.0.1:9015',
    },
  },
  build: {
    rollupOptions: {
      input: {
        index: 'index.html',
        archive: 'archive.html',
        post: 'post.html',
        admin: 'admin.html',
      },
    },
  },
});
