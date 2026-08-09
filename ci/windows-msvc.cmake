# Rust uses the non-debug dynamic CRT in every Cargo profile. Keep CMake-built
# dependencies on the same runtime so dev and test binaries do not mix CRTs.
set(CMAKE_POLICY_DEFAULT_CMP0091 NEW CACHE STRING "" FORCE)
set(CMAKE_MSVC_RUNTIME_LIBRARY MultiThreadedDLL CACHE STRING "" FORCE)
