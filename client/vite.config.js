import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [tailwindcss(), svelte()],
	server: {
		https: false,
		proxy: {
			"/api": {
				target: "http://127.0.0.1:3000",
				// target: 'https://wewerewondering.com',
				changeOrigin: true,
				secure: false
			}
		}
	}
});
