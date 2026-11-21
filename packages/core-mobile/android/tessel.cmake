# Links tessel-core's mobile binding (../rust, tessel-mobile) and its JSI module into a
# React Native app's native library. `ream android:*` does it from this package's
# reamNative declaration:
#
#   include(${TESSEL_DIR}/android/tessel.cmake)
#   tessel_link(${CMAKE_PROJECT_NAME})
#
# and registers NativeTessel in the app's cxxModuleProvider.

set(TESSEL_ANDROID_DIR ${CMAKE_CURRENT_LIST_DIR})

# `ream android:*` builds the Rust crate into REAM_RUST_DIR/<abi>/.
set(TESSEL_MOBILE_LIB ${REAM_RUST_DIR}/${ANDROID_ABI}/libtessel_mobile.a)

function(tessel_link target)
  target_sources(${target} PRIVATE ${TESSEL_ANDROID_DIR}/src/main/cpp/NativeTessel.cpp)
  target_include_directories(${target} PUBLIC ${TESSEL_ANDROID_DIR}/src/main/cpp)
  add_library(tessel_mobile STATIC IMPORTED)
  set_target_properties(tessel_mobile PROPERTIES IMPORTED_LOCATION ${TESSEL_MOBILE_LIB})
  target_link_libraries(${target} tessel_mobile)
endfunction()
