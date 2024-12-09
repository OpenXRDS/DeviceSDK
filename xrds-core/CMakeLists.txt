project(xrds-core)

file(GLOB_RECURSE XRDS_CORE_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")
add_rust_library(
    TARGET ${PROJECT_NAME}
    ALIAS xrds::core
    SOURCES ${XRDS_CORE_SOURCES}
)
