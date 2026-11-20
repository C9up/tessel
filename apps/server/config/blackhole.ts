import { defineConfig } from "@c9up/blackhole/config";
import env from "#start/env.js";

export default defineConfig({
	xss: true,
	csrf: { exceptRoutes: [] },
	// Through the validated environment, never `process.env` directly: the
	// schema in start/env.ts is what guarantees the key is there at all, and
	// reading around it gives back `string | undefined` typed as a string.
	// Released here, where the raw key is needed, and nowhere else.
	secret: env.get("APP_KEY").release(),
});
