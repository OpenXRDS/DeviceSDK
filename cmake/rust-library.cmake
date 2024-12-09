function(add_rust_library)
    set(options)
    set(oneValueArgs TARGET ALIAS)
    set(multiValueArgs SOURCES)
    cmake_parse_arguments(PARSE_ARGV 0 DRM_ARG
        "${options}"
        "${oneValueArgs}"
        "${multiValueArgs}"
    )

    if(NOT DRM_ARG_TARGET)
        message(FATAL_ERROR "DefineRustModule TARGET must be presented")
    endif()

    if(NOT DRM_ARG_SOURCES)
        message(FATAL_ERROR "DefineRustModule SOURCES must be presented")
    endif()

    add_library(${DRM_ARG_TARGET} INTERFACE ${DRM_ARG_SOURCES})

    if(DRM_ARG_ALIAS)
        add_library(${DRM_ARG_ALIAS} ALIAS ${DRM_ARG_TARGET})
    endif()

    string(TOLOWER ${DRM_ARG_TARGET} DRM_LIBNAME)
    string(REPLACE "-" "_" DRM_LIBNAME ${DRM_LIBNAME})

    if(WIN32)
        set(${DRM_ARG_TARGET}_DYLIB_NAME "${DRM_LIBNAME}.dll")
        set(${DRM_ARG_TARGET}_LIB_NAME "${DRM_LIBNAME}.dll.lib")
    elseif(IOS OR APPLE)
        set(${DRM_ARG_TARGET}_DYLIB_NAME "${DRM_LIBNAME}.dylib")
        set(${DRM_ARG_TARGET}_LIB_NAME "${DRM_LIBNAME}.lib")
    elseif(LINUX)
        set(${DRM_ARG_TARGET}_DYLIB_NAME "lib${DRM_LIBNAME}.so")
        set(${DRM_ARG_TARGET}_LIB_NAME "${DRM_LIBNAME}.a")
    endif()

    target_sources(${DRM_ARG_TARGET}
        PRIVATE
        $<IF:$<CONFIG:Debug>,${CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG},${CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE}>/${${DRM_ARG_TARGET}_DYLIB_NAME}
    )

    target_include_directories(${DRM_ARG_TARGET}
        INTERFACE
        "${CMAKE_CURRENT_SOURCE_DIR}/include"
    )

    target_link_libraries(${DRM_ARG_TARGET}
        INTERFACE
        ${CMAKE_BINARY_DIR}/rust-target/$<IF:$<CONFIG:Debug>,debug,release>/${${DRM_ARG_TARGET}_LIB_NAME}
    )

    add_custom_command(OUTPUT $<IF:$<CONFIG:Debug>,${CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG},${CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE}>/${${DRM_ARG_TARGET}_DYLIB_NAME}
        COMMAND cargo build $<$<NOT:$<CONFIG:Debug>>:--release>
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
        ${CMAKE_BINARY_DIR}/rust-target/$<IF:$<CONFIG:Debug>,debug,release>/${${DRM_ARG_TARGET}_DYLIB_NAME}
        $<IF:$<CONFIG:Debug>,${CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG},${CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE}>/${${DRM_ARG_TARGET}_DYLIB_NAME}
        COMMAND_EXPAND_LISTS
        DEPENDS ${DRM_ARG_SOURCES}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
        VERBATIM
    )

    install(
        FILES "$<IF:$<CONFIG:Debug>,${CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG},${CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE}>/${${DRM_ARG_TARGET}_DYLIB_NAME}"
        DESTINATION $<IF:$<CONFIG:Debug>,bin/debug,bin/release>
        PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE
    )

    install(
        FILES "${CMAKE_BINARY_DIR}/rust-target/$<IF:$<CONFIG:Debug>,debug,release>/${${DRM_ARG_TARGET}_LIB_NAME}"
        DESTINATION $<IF:$<CONFIG:Debug>,lib/debug,lib/release>
        PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE
    )

    install(
        DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/include"
        DESTINATION "."
        FILES_MATCHING PATTERN "*.h" PATTERN "*.hpp"
        PERMISSIONS OWNER_READ OWNER_WRITE GROUP_READ GROUP_WRITE WORLD_READ
    )
endfunction(add_rust_library)
