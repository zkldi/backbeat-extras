import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
	root: fileURLToPath(new URL("./frontend", import.meta.url)),
	build: {
		outDir: "../dist",
		emptyOutDir: true,
	},
	plugins: [solid()],
	clearScreen: false,
	server: {
		fs: { allow: [fileURLToPath(new URL(".", import.meta.url))] },
		port: 1422,
		strictPort: true,
		watch: { ignored: ["**/src-tauri/**"] },
	},
});
