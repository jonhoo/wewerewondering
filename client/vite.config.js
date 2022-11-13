import postcss from './postcss.config.cjs';
import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [svelte()],
  server: {
    https: false,
    proxy: {
      '/api': {
          target: 'http://127.0.0.1:3000',
          // target: 'https://wewerewondering.com',
          changeOrigin: true,
          secure: false
      }
    }
  },
  css:{
    postcss
  }
})
