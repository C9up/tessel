/**
 * Encrypted profiles (story 03-06). The server keeps one blob per account, sealed by its
 * devices under the account's profile key; it checks the blob's version and size, and
 * nothing else, for there is nothing else it can read.
 */

/** Version of a sealed profile, its first byte (`tessel-core`'s `PROFILE_VERSION`). */
export const PROFILE_VERSION = 1

/** A name and a 64 KiB avatar, padded and sealed, stay well under this. */
export const MAX_PROFILE_BLOB = 80 * 1024

/** Whether `blob` can be a sealed profile: the version, a nonce and a tag at least. */
export function isProfileBlob(blob: Uint8Array): boolean {
  return blob.length > 1 + 24 + 16 && blob.length <= MAX_PROFILE_BLOB && blob[0] === PROFILE_VERSION
}
