import app from "@c9up/ream/services/app";
import env from "#start/env.js";

const user = encodeURIComponent(env.get("DB_USER"));
const password = encodeURIComponent(env.get("DB_PASSWORD") ?? "");

export default {
	url: `postgres://${user}:${password}@${env.get("DB_HOST")}:${env.get("DB_PORT")}/${env.get("DB_DATABASE")}`,
	migrations: {
		// Paresseux : `app.migrationsPath()` lève tant que la racine de l'app n'est pas posée.
		get paths() {
			return [app.migrationsPath()];
		},
		// Une seule instance en production pour l'instant : le conteneur migre au démarrage
		// (ignoré en développement, où l'on lance `pnpm migrate`). À retirer le jour où le
		// serveur passe à plusieurs répliques (story 04-05).
		autoRun: true,
	},
};
