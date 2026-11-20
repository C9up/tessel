import { Env } from "@c9up/ream";

export default await Env.create(new URL("../", import.meta.url), {
	APP_NAME: Env.schema.string(),
	// Signe cookies et jetons. Obligatoire : sans elle l'app refuse de démarrer.
	APP_KEY: Env.schema.secret(),
	NODE_ENV: Env.schema.enum(["development", "production", "test"] as const),
	PORT: Env.schema.number(),
	DB_CONNECTION: Env.schema.enum(["postgres"] as const),
	DB_HOST: Env.schema.string({ format: "host" }),
	DB_PORT: Env.schema.number(),
	DB_DATABASE: Env.schema.string(),
	DB_USER: Env.schema.string(),
	DB_PASSWORD: Env.schema.string.optional(),
	REDIS_URL: Env.schema.string(),
});
