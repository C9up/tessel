import "reflect-metadata";
import { Ignitor, prettyPrintError } from "@c9up/ream";

const APP_ROOT = new URL("../", import.meta.url);

const IMPORTER = (filePath: string) =>
	filePath.startsWith("./") || filePath.startsWith("../")
		? import(new URL(filePath, APP_ROOT).href)
		: import(filePath);

new Ignitor(APP_ROOT, { importer: IMPORTER })
	.tap((app) => {
		// Validates the environment against start/env.ts before anything boots, so
		// a missing APP_KEY stops the app here rather than on the first response
		// that tries to sign a cookie.
		app.booting(async () => {
			await import("#start/env.js");
		});
	})
	.useRcFile((await import("../reamrc.js")).default)
	.console()
	.handle(process.argv.slice(2))
	.catch((err) => {
		prettyPrintError(err);
		process.exit(1);
	});
