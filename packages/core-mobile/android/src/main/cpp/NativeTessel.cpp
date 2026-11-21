#include "NativeTessel.h"

#include <fbjni/fbjni.h>
#include <jni.h>

#include <cstdint>
#include <mutex>
#include <optional>
#include <string>
#include <vector>

#include "tessel_mobile.h"

namespace facebook::react {

namespace {

// The Kotlin side (com.c9up.tessel.TesselPlatform), found once through the app's own
// class loader: a FindClass from the JS thread would only see the system's.
jclass gPlatform = nullptr;
jmethodID gSeal = nullptr;
jmethodID gUnseal = nullptr;
std::once_flag gConfigured;
std::string gConfigureError;

// The running Application, through ActivityThread: the package declares no hook that
// runs at the app's start (reamNative has none yet; reported to Ream).
jobject currentApplication(JNIEnv* env) {
  jclass thread = env->FindClass("android/app/ActivityThread");
  jmethodID current = env->GetStaticMethodID(thread, "currentApplication", "()Landroid/app/Application;");
  jobject app = env->CallStaticObjectMethod(thread, current);
  env->DeleteLocalRef(thread);
  return app;
}

jclass loadPlatform(JNIEnv* env, jobject app) {
  jclass contextClass = env->GetObjectClass(app);
  jmethodID getLoader = env->GetMethodID(contextClass, "getClassLoader", "()Ljava/lang/ClassLoader;");
  jobject loader = env->CallObjectMethod(app, getLoader);
  jclass loaderClass = env->GetObjectClass(loader);
  jmethodID loadClass = env->GetMethodID(loaderClass, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;");
  jstring name = env->NewStringUTF("com.c9up.tessel.TesselPlatform");
  auto platform = static_cast<jclass>(env->CallObjectMethod(loader, loadClass, name));
  env->DeleteLocalRef(name);
  env->DeleteLocalRef(loaderClass);
  env->DeleteLocalRef(loader);
  env->DeleteLocalRef(contextClass);
  return platform;
}

// TesselSealFn: the Keystore's seal or unseal, through TesselPlatform.
int64_t sealWithKeystore(int32_t unseal, const uint8_t* in, size_t len, uint8_t* out, size_t outCap) {
  JNIEnv* env = facebook::jni::Environment::current();
  jbyteArray input = env->NewByteArray(static_cast<jsize>(len));
  env->SetByteArrayRegion(input, 0, static_cast<jsize>(len), reinterpret_cast<const jbyte*>(in));
  auto output = static_cast<jbyteArray>(env->CallStaticObjectMethod(gPlatform, unseal != 0 ? gUnseal : gSeal, input));
  env->DeleteLocalRef(input);
  if (env->ExceptionCheck() || output == nullptr) {
    // A key that does not open (another phone, a wiped Keystore) is a refusal, not a crash.
    env->ExceptionClear();
    return -1;
  }
  jsize written = env->GetArrayLength(output);
  if (static_cast<size_t>(written) > outCap) {
    env->DeleteLocalRef(output);
    return -1;
  }
  env->GetByteArrayRegion(output, 0, written, reinterpret_cast<jbyte*>(out));
  env->DeleteLocalRef(output);
  return written;
}

void configure() {
  JNIEnv* env = facebook::jni::Environment::current();
  jobject app = currentApplication(env);
  if (app == nullptr) {
    gConfigureError = "host:no-application\nno running Android application";
    return;
  }
  jclass platform = loadPlatform(env, app);
  if (env->ExceptionCheck() || platform == nullptr) {
    env->ExceptionClear();
    env->DeleteLocalRef(app);
    gConfigureError = "host:no-platform\ncom.c9up.tessel.TesselPlatform is not in the app";
    return;
  }
  gPlatform = static_cast<jclass>(env->NewGlobalRef(platform));
  gSeal = env->GetStaticMethodID(gPlatform, "seal", "([B)[B");
  gUnseal = env->GetStaticMethodID(gPlatform, "unseal", "([B)[B");
  jmethodID filesDir = env->GetStaticMethodID(gPlatform, "filesDir", "(Landroid/content/Context;)Ljava/lang/String;");
  auto dir = static_cast<jstring>(env->CallStaticObjectMethod(gPlatform, filesDir, app));
  const char* chars = env->GetStringUTFChars(dir, nullptr);
  std::string path(chars);
  env->ReleaseStringUTFChars(dir, chars);
  env->DeleteLocalRef(dir);
  env->DeleteLocalRef(platform);
  env->DeleteLocalRef(app);
  if (tessel_configure(reinterpret_cast<const uint8_t*>(path.data()), path.size(), &sealWithKeystore) != 0) {
    gConfigureError = "host:configure\nthe core could not open its directory";
  }
}

// The bytes of an ArrayBuffer, or of a typed array's view on one.
std::vector<uint8_t> bytesOf(jsi::Runtime& rt, jsi::Object& value) {
  const char* refused = "input:malformed\narguments must be a Uint8Array or an ArrayBuffer";
  std::optional<jsi::ArrayBuffer> buffer;
  size_t offset = 0;
  size_t length = 0;
  if (value.isArrayBuffer(rt)) {
    buffer.emplace(value.getArrayBuffer(rt));
    length = buffer->size(rt);
  } else {
    jsi::Value inner = value.getProperty(rt, "buffer");
    jsi::Value byteOffset = value.getProperty(rt, "byteOffset");
    jsi::Value byteLength = value.getProperty(rt, "byteLength");
    if (!inner.isObject() || !byteOffset.isNumber() || !byteLength.isNumber()) throw jsi::JSError(rt, refused);
    jsi::Object object = inner.getObject(rt);
    if (!object.isArrayBuffer(rt) || byteOffset.getNumber() < 0 || byteLength.getNumber() < 0) throw jsi::JSError(rt, refused);
    buffer.emplace(object.getArrayBuffer(rt));
    offset = static_cast<size_t>(byteOffset.getNumber());
    length = static_cast<size_t>(byteLength.getNumber());
  }
  size_t size = buffer->size(rt);
  if (offset > size || length > size - offset) throw jsi::JSError(rt, refused);
  const uint8_t* data = buffer->data(rt) + offset;
  return std::vector<uint8_t>(data, data + length);
}

// An ArrayBuffer that owns the core's answer.
class Answer : public jsi::MutableBuffer {
 public:
  explicit Answer(std::vector<uint8_t> bytes) : bytes_(std::move(bytes)) {}
  size_t size() const override { return bytes_.size(); }
  uint8_t* data() override { return bytes_.data(); }

 private:
  std::vector<uint8_t> bytes_;
};

} // namespace

NativeTessel::NativeTessel(std::shared_ptr<CallInvoker> jsInvoker)
    : NativeTesselCxxSpec(std::move(jsInvoker)) {}

jsi::Object NativeTessel::call(jsi::Runtime& rt, std::string method, jsi::Object args) {
  std::call_once(gConfigured, configure);
  if (!gConfigureError.empty()) throw jsi::JSError(rt, gConfigureError);
  std::vector<uint8_t> input = bytesOf(rt, args);
  TesselBuf out{};
  int32_t code = tessel_call(
      reinterpret_cast<const uint8_t*>(method.data()), method.size(), input.data(), input.size(), &out);
  std::vector<uint8_t> answer(out.ptr, out.ptr + out.len);
  tessel_buf_free(out);
  if (code != 0) {
    // "<code>\n<message>": @tessel/core-mobile turns it back into a TesselError.
    throw jsi::JSError(rt, std::string(answer.begin(), answer.end()));
  }
  return jsi::ArrayBuffer(rt, std::make_shared<Answer>(std::move(answer)));
}

} // namespace facebook::react
