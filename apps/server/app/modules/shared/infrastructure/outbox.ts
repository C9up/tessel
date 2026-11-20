/**
 * Le côté durable de l'outbox (repris de nadeo, sans tenant).
 *
 * Des fonctions sur une poignée `OutboxDb` minimale, que la vraie connexion atlas
 * satisfait par sa forme : les tests passent un double en mémoire sans cast.
 *
 * Toute l'heure est celle de la BASE (`now()`) : ni l'horloge du client ni un
 * décalage entre client et base n'entrent en jeu. Chaque paramètre porte un cast
 * explicite (`::uuid`, `::jsonb`, `::double precision`), le pilote liant une chaîne
 * JS en `text`.
 */
export interface OutboxRow {
	id: string;
	event_type: string;
	/** jsonb : le pilote peut le rendre déjà analysé OU en chaîne JSON. */
	payload: unknown;
	attempts: number;
}

/** La surface de base dont l'outbox a besoin : un sous-ensemble de la connexion atlas. */
export interface OutboxDb {
	query(sql: string, params?: unknown[]): Promise<OutboxRow[]>;
	execute(sql: string, params?: unknown[]): Promise<{ rowsAffected: number }>;
}

export interface OutboxEntry {
	id: string;
	eventType: string;
	payload: unknown;
	/** Secondes avant que le relais puisse prendre la ligne (délai laissé au chemin rapide). */
	delaySeconds: number;
}

/** Garder un événement en attente, que le relais pourra prendre après `delaySeconds`. */
export async function enqueueOutbox(
	db: OutboxDb,
	entry: OutboxEntry,
): Promise<void> {
	await db.execute(
		`INSERT INTO outbox (id, event_type, payload, created_at, available_at, attempts)
     VALUES ($1::uuid, $2, $3::jsonb, now(), now() + make_interval(secs => $4::double precision), 0)`,
		[
			entry.id,
			entry.eventType,
			JSON.stringify(entry.payload),
			entry.delaySeconds,
		],
	);
}

/**
 * RÉSERVER au plus `limit` lignes dues : `FOR UPDATE SKIP LOCKED` prend celles qu'aucun
 * autre relais ne tient, et la même requête repousse leur `available_at` de
 * `leaseSeconds`. Un relais qui meurt en pleine livraison laisse le bail expirer : la
 * ligne redevient due.
 */
export function claimOutboxBatch(
	db: OutboxDb,
	opts: { limit: number; leaseSeconds: number },
): Promise<OutboxRow[]> {
	return db.query(
		`UPDATE outbox SET available_at = now() + make_interval(secs => $2::double precision)
     WHERE id IN (
       SELECT id FROM outbox
       WHERE available_at <= now()
       ORDER BY created_at
       LIMIT $1
       FOR UPDATE SKIP LOCKED
     )
     RETURNING id, event_type, payload, attempts`,
		[opts.limit, opts.leaseSeconds],
	);
}

/** Retirer une ligne livrée : l'outbox ne garde que le travail en cours. */
export async function deleteOutbox(db: OutboxDb, id: string): Promise<void> {
	await db.execute("DELETE FROM outbox WHERE id = $1::uuid", [id]);
}

/** Compter un échec, noter l'erreur, et repousser la prochaine tentative de `delaySeconds`. */
export async function rescheduleOutbox(
	db: OutboxDb,
	id: string,
	opts: { delaySeconds: number; error: string },
): Promise<void> {
	await db.execute(
		`UPDATE outbox SET attempts = attempts + 1,
        available_at = now() + make_interval(secs => $2::double precision),
        last_error = $3
     WHERE id = $1::uuid`,
		[id, opts.delaySeconds, opts.error.slice(0, 2000)],
	);
}
