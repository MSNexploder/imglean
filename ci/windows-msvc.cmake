# Rust uses the non-debug dynamic CRT in every Cargo profile. Keep CMake-built
# dependencies on the same runtime so dev and test binaries do not mix CRTs.
set(CMAKE_POLICY_DEFAULT_CMP0091 NEW CACHE STRING "" FORCE)
set(CMAKE_MSVC_RUNTIME_LIBRARY MultiThreadedDLL CACHE STRING "" FORCE)

# Older CMake projects can retain /MDd in their Debug flag cache even when the
# runtime policy above is requested. Keep all other Debug behavior while
# selecting the same dynamic non-debug CRT as Rust.
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
  set(CMAKE_C_FLAGS_DEBUG "/MD /Zi /Ob0 /Od /RTC1 /D_DEBUG" CACHE STRING "" FORCE)
  set(CMAKE_CXX_FLAGS_DEBUG "/MD /Zi /Ob0 /Od /RTC1 /D_DEBUG" CACHE STRING "" FORCE)
endif()
