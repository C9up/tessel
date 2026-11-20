import { DateTime } from "@c9up/chronos";

/**
 * La base de tout événement du domaine (modèle nadeo) : un `eventId` stable,
 * clé d'idempotence, et l'instant où il s'est produit. Pur : `emit()` le remet
 * au publieur que l'infrastructure branche au démarrage
 * (`shared/infrastructure/events.ts`), qui le garde dans l'outbox avant que les
 * écouteurs ne l'entendent.
 *
 * Un événement ne porte JAMAIS de contenu de message : le serveur n'en a pas.
 * Ce qu'il porte s'inscrit au registre des métadonnées (story 15-02).
 */
export abstract class DomainEvent {
	/** Identifiant stable de l'instance : la clé de déduplication. */
	readonly eventId: string = globalThis.crypto.randomUUID();
	/** Instant ISO-8601 de la construction de l'événement. */
	readonly occurredAt: string = DateTime.now().toISO();

	/** Publié : gardé durablement d'abord, puis livré aux écouteurs du processus. */
	emit(): Promise<void> {
		return publish(this);
	}
}

let publish: (event: DomainEvent) => Promise<void> = async () => {};

/** Branché une fois au démarrage par l'infrastructure, qui possède l'outbox et l'émetteur. */
export function setEventPublisher(
	fn: (event: DomainEvent) => Promise<void>,
): void {
	publish = fn;
}
