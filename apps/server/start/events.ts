// The wiring of events between modules lives here, and only here: a module publishes
// its events, `start/` ties an event to a listener already written (`onDomainEvent`).
// Importing the events infrastructure also plugs in the domain's publisher.
import '#modules/shared/infrastructure/events.js'
