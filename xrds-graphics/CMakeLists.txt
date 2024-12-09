project(xrds-graphics)

file(GLOB_RECURSE XRDS_GRAPHICS_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")
add_rust_library(
    TARGET ${PROJECT_NAME}
    ALIAS xrds::graphics
    SOURCES ${XRDS_GRAPHICS_SOURCES}
)
