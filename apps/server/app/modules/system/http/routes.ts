import router from "@c9up/ream/services/router";

// Sonde de vie, pour l'hébergeur et pour les clients.
router.get("/health", async ({ response }) => {
	response.status(200).json({ name: "tessel", status: "running" });
});
