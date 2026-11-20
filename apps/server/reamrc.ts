import { defineConfig } from "@c9up/ream";

export default defineConfig({
	providers: [
		() => import("@c9up/spectrum/provider"),
		() => import("@c9up/atlas/provider"),
		() => import("@c9up/ream/events/provider"),
		() => import("@c9up/quasar/provider"),
		() => import("@c9up/blackhole/provider"),
		() => import("#providers/AppProvider.js"),
	],
	// Les commandes viennent des paquets, jamais du binaire global (absent d'une CI).
	commands: [() => import("@c9up/atlas/commands")],
	preloads: [
		() => import("#start/kernel.js"),
		() => import("#start/events.js"),
	],
	modules: {
		path: "./app/modules",
		// Un module garde ses routes dans sa couche http (modèle nadeo).
		autoload: ["http/routes"],
	},
	tests: {
		suites: [{ name: "app", files: ["app/modules/**/__tests__/**/*.test.ts"] }],
	},
});
