project(xrds-components)

file(GLOB_RECURSE XRDS_COMPONENTS_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")
add_rust_library(
    TARGET ${PROJECT_NAME}
    ALIAS xrds::components
    SOURCES ${XRDS_COMPONENTS_SOURCES}
)
