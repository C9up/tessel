import { Migration } from '@c9up/atlas'

/**
 * The KeyPackage directory (story 03-03). A KeyPackage holds public keys only; the
 * server keeps it, its device, its kind and the day its validity ends. Each column is in
 * the metadata register (story 15-02).
 */
export default class CreateKeyPackages extends Migration {
  async up() {
    this.schema.raw(`
      CREATE TABLE key_packages (
        -- KeyPackageRef (RFC 9420, 5.2): a hash of the KeyPackage.
        reference bytea PRIMARY KEY,
        device_id bytea NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
        -- The MLSMessage, as the device made it: public keys and its certificate.
        key_package bytea NOT NULL,
        last_resort boolean NOT NULL,
        -- The day its validity ends (the day only, ADR-006).
        not_after date NOT NULL
      )
    `)
    this.schema.raw('CREATE INDEX key_packages_device_idx ON key_packages (device_id, last_resort, not_after)')
  }

  async down() {
    this.schema.raw('DROP TABLE key_packages')
  }
}
