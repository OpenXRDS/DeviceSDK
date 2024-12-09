project(xrds-audio)

file(GLOB_RECURSE XRDS_AUDIO_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs")
add_rust_library(
    TARGET ${PROJECT_NAME}
    ALIAS xrds::audio
    SOURCES ${XRDS_AUDIO_SOURCES}
)
