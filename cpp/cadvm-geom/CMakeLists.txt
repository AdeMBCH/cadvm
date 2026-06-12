cmake_minimum_required(VERSION 3.16)
project(cadvm_geom CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Open CASCADE Technology. The Ubuntu `libocct-*-dev` packages ship a CMake
# package config, so find_package locates headers and toolkit libraries.
find_package(OpenCASCADE REQUIRED)

add_executable(cadvm-geom src/main.cpp)

target_include_directories(cadvm-geom PRIVATE ${OpenCASCADE_INCLUDE_DIR})

# Toolkits needed for: STEP import (TKSTEP/TKXSBase), B-Rep + geometry
# (TKBRep/TKMath/TKG2d/TKG3d/TKGeomBase), and boolean ops (TKTopAlgo/TKBO/TKPrim),
# plus the foundation (TKernel).
target_link_libraries(cadvm-geom PRIVATE
    TKSTEP
    TKXSBase
    TKBRep
    TKMath
    TKG2d
    TKG3d
    TKGeomBase
    TKGeomAlgo
    TKTopAlgo
    TKPrim
    TKBO
    TKMesh
    TKernel
)

install(TARGETS cadvm-geom RUNTIME DESTINATION bin)
