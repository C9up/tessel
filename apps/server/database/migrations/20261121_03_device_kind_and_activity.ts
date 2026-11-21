import { Migration } from '@c9up/atlas'

/**
 * The device list (story 03-05): its kind, as the device declares it when it signs in,
 * and the last day it was active (the day, never the hour). Both are in the metadata
 * register (story 15-02).
 */
export default class DeviceKindAndActivity extends Migration {
  async up() {
    // web, android or ios: the server learns it anyway from the push service a device
    // registers with (epic 06).
    this.schema.raw(`ALTER TABLE devices ADD COLUMN kind text CHECK (kind IN ('web', 'android', 'ios'))`)
    this.schema.raw('ALTER TABLE devices ADD COLUMN last_active_on date')
  }

  async down() {
    this.schema.raw('ALTER TABLE devices DROP COLUMN last_active_on')
    this.schema.raw('ALTER TABLE devices DROP COLUMN kind')
  }
}
