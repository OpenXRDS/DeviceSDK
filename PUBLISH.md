flowchart TD
    A[GUI Editor] --> B[Author Scene Content]
    B --> C[XrdsSceneDocument]
    B --> D[Asset Catalog and Project Assets]
    B --> E[App or Project Manifest]

    C --> F[Export XRDS Content Package]
    D --> F
    E --> F

    F --> G[Publish Pipeline]
    G --> H[Select Target Device Profile]

    H --> I[Generic XR Runner Template]
    F --> I

    I --> J[Build and Package]
    J --> K[Installable XR Application]

    K --> L[Deploy to XR Device]
    L --> M[Launch OpenXR Runtime]
    M --> N[Runner Loads XRDS Content]
    N --> O[XrdsAPI Imports Scene]
    O --> P[Live XR World on Device]

    Q[Optional App Logic or Gameplay] --> I
    R[Optional Platform Signing and Store Packaging] --> J


* The GUI editor authors content, not the final executable directly.
* The editor exports an XRDS content package: scene document, assets, manifest.
* A publish pipeline combines that package with a generic XR runner template.
* The build step creates an installable XR application for the target device.
* On device, the runner starts OpenXR, loads the content, and imports it through [XrdsAPI](vscode-file://vscode-app/c:/Program%20Files/Microsoft%20VS%20Code/41dd792b5e/resources/app/out/vs/code/electron-browser/workbench/workbench.html).
