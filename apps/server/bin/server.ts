import "reflect-metadata";
import { Ignitor, prettyPrintError } from "@c9up/ream";
import { createHyperServerFactory } from "@c9up/ream/bootstrap";
import { setOutboxActive } from "#modules/shared/infrastructure/events.js";
import { outboxRelay } from "#modules/shared/infrastructure/outboxRelay.js";

const APP_ROOT = new URL("../", import.meta.url);

// PORT n'est pas lu ici : `#start/env` charge `.env` dans `booting()`, et l'Ignitor
// lit le port au moment de se lier.
new Ignitor(APP_ROOT, {
	serverFactory: createHyperServerFactory(),
})
	.tap((app) => {
		// Valider l'environnement avant tout démarrage : une APP_KEY absente arrête l'app ici.
		app.booting(async () => {
			await import("#start/env.js");
		});
	})
	.useRcFile((await import("../reamrc.js")).default)
	.httpServer()
	.start()
	.then(async (app) => {
		// L'outbox et son relais ne tournent que dans le vrai serveur, jamais sous le harnais de test.
		setOutboxActive(true);
		outboxRelay.start();
		console.log(
			`\n  ➜ Tessel ready on http://${app.host()}:${await app.port()}\n`,
		);
	})
	.catch((err) => {
		prettyPrintError(err);
		process.exit(1);
	});
