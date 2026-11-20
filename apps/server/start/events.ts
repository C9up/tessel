// Le câblage des événements entre modules vit ici, et seulement ici : un module publie
// ses événements, `start/` relie un événement à un écouteur déjà écrit (`onDomainEvent`).
// Importer l'infrastructure des événements branche aussi le publieur du domaine.
import '#modules/shared/infrastructure/events.js'
