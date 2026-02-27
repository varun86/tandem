import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
    plugins: [react(), tailwindcss()],
    server: {
        proxy: {
            "/engine": {
                target: process.env.VITE_TANDEM_ENGINE_URL || "http://127.0.0.1:39731",
                changeOrigin: true,
                rewrite: (path) => path.replace(/^\/engine/, ""),
            },
        },
    },
});
