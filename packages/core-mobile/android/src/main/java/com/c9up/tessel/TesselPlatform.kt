package com.c9up.tessel

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * What tessel-core asks of Android (story 02-13), called from the C++ module through
 * JNI: where the app keeps its files, and the Android Keystore that seals the local
 * database's key (ADR-007). The Keystore key is AES-256-GCM, generated inside the
 * Keystore and never exportable: the sealed store key is useless off this phone.
 */
object TesselPlatform {
  private const val KEYSTORE = "AndroidKeyStore"
  private const val ALIAS = "tessel-store-key-seal"
  private const val TRANSFORMATION = "AES/GCM/NoPadding"
  private const val NONCE_BYTES = 12
  private const val TAG_BITS = 128

  /** The app's private files directory: where the database lives. */
  @JvmStatic
  fun filesDir(context: Context): String = context.filesDir.absolutePath

  /** `data` sealed with the Keystore key: the nonce, then the ciphertext and its tag. */
  @JvmStatic
  fun seal(data: ByteArray): ByteArray {
    val cipher = Cipher.getInstance(TRANSFORMATION)
    cipher.init(Cipher.ENCRYPT_MODE, key())
    val nonce = cipher.iv
    check(nonce.size == NONCE_BYTES) { "unexpected nonce size" }
    return nonce + cipher.doFinal(data)
  }

  /** What [seal] sealed; throws when it does not open (another phone, a deleted key). */
  @JvmStatic
  fun unseal(sealed: ByteArray): ByteArray {
    require(sealed.size > NONCE_BYTES) { "sealed data too short" }
    val cipher = Cipher.getInstance(TRANSFORMATION)
    cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(TAG_BITS, sealed, 0, NONCE_BYTES))
    return cipher.doFinal(sealed, NONCE_BYTES, sealed.size - NONCE_BYTES)
  }

  /** The Keystore key, generated on first use. */
  @Synchronized
  private fun key(): SecretKey {
    val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }
    (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
    val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
    generator.init(
      KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setKeySize(256)
        .setRandomizedEncryptionRequired(true)
        .build(),
    )
    return generator.generateKey()
  }
}
