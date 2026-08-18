# Publish Pipeline

flowchart TD
    A[GUI Editor] --> B[Author Scene Content]
    B --> C[XrdsSceneDocument]
    B --> D[Asset Catalog and Project Assets]

    C --> E[Export as Application]
    D --> E

    E --> F[Generated Runner Crate]
    F --> F1[src/main.rs — XrdsSceneViewer]
    F --> F2[Cargo.toml — SDK path dependency]
    F --> F3[assets/ — scene.xrds + all referenced assets]

    F --> G[cargo build --release]
    G --> H[Distributable Binary + assets/]
    H --> I[Reveal in Explorer — immediately runnable]

    subgraph Future
        J[App or Project Manifest] --> K[Publish Pipeline]
        K --> L[Select Target Device Profile]
        L --> M[Platform Packaging and Signing]
        M --> N[Store or Sideload Package]
        N --> O[Deploy to XR Device]
        O --> P[Launch OpenXR Runtime]
        P --> Q[Runner Loads XRDS Content]
        Q --> R[Live XR World on Device]
    end

    I -.->|future path| K

* The GUI editor authors content and exports a self-contained runner project.
* Export bundles the scene document, all referenced assets, and a generated Rust runner (XrdsSceneViewer).
* The runner is built locally with `cargo build --release` — no separate build server required.
* The resulting binary + assets/ folder is immediately runnable by double-click on the host platform.
* Validated on Windows, Linux (Ubuntu), and macOS.

## Current Export Flow (Implemented)

    File → Export as Application…
      └─ folder picker
           └─ clone scene, relativize asset URIs
                └─ copy all catalog assets to assets/
                     └─ generate Cargo.toml + src/main.rs
                          └─ cargo build --release (background thread)
                               └─ copy assets/ to target/release/assets/
                                    └─ reveal target/release/ in explorer

## Future: Publish Pipeline (Not Yet Implemented)

To ship to XR devices (Meta Quest, Pico, VIVE, etc.) the following is needed on top of the current export:

* **App manifest** — display name, version, permissions, orientation, min/target API level
* **Target device profile** — per-platform feature flags and dependency gates
* **Platform packaging** — APK (Android/Quest), IPA (visionOS), or installer (PC VR)
* **Signing** — developer certificate, store signing, or debug signing for sideload
* **Store packaging** — App Lab / Meta Store / Steam submission bundle
* **Device deploy** — ADB sideload (Android) or platform deploy tool

The runner template pattern (generate → build → package) established in Phase 1 is the foundation this pipeline will build on.
