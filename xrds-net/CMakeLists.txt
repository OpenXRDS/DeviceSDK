project(xrds-net)

file(GLOB_RECURSE XRDS_NET_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")
add_rust_library(
    TARGET ${PROJECT_NAME}
    ALIAS xrds::net
    SOURCES ${XRDS_NET_SOURCES}
)
