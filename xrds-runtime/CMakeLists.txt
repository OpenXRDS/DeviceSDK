project(xrds-runtime)

file(GLOB_RECURSE XRDS_RUNTIME_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")
add_rust_library(
    TARGET ${PROJECT_NAME}
    ALIAS xrds::runtime
    SOURCES ${XRDS_RUNTIME_SOURCES}
)
