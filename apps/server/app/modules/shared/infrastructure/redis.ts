import type { QuasarConnection } from "@c9up/quasar";
import quasar from "@c9up/quasar/services/main";

/**
 * La connexion KeyDB partagée, servie par `@c9up/quasar`, en accesseur paresseux :
 * importer ce fichier n'ouvre aucun socket, la connexion n'est résolue qu'à l'appel
 * d'une méthode (repris de nadeo).
 */
export const redis: QuasarConnection = new Proxy(Object.create(null), {
	get(_target, prop) {
		const connection = quasar.connection();
		const value = Reflect.get(connection, prop, connection);
		return typeof value === "function" ? value.bind(connection) : value;
	},
});
