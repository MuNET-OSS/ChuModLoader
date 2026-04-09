cmake_minimum_required(VERSION 3.15)
project(ChuModLoader LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_SHARED_LIBRARY_PREFIX "")

add_library(ChuModLoader SHARED
    src/main.cpp
    src/loader.cpp
    src/version.def
)

target_include_directories(ChuModLoader PRIVATE include)

set_target_properties(ChuModLoader PROPERTIES
    OUTPUT_NAME "version"
    SUFFIX ".dll"
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
)
