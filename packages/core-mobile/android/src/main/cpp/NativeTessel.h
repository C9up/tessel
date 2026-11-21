#pragma once

#include <AppSpecsJSI.h>

#include <memory>
#include <string>

namespace facebook::react {

// tessel-core over JSI (story 02-13): see specs/NativeTessel.ts.
class NativeTessel : public NativeTesselCxxSpec<NativeTessel> {
 public:
  explicit NativeTessel(std::shared_ptr<CallInvoker> jsInvoker);

  jsi::Object call(jsi::Runtime& rt, std::string method, jsi::Object args);
};

} // namespace facebook::react
