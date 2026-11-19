//! Release notes for the Rust edition, newest first. Every update or bugfix bumps the patch
//! version (v0.0.1 → v0.0.2 …) — keep `Cargo.toml` in sync.

pub type Release = (&'static str, &'static str, &'static str, &'static [(&'static str, &'static str)]);

pub const CHANGELOG: &[Release] = &[
    (
        "0.0.15",
        "2026-11-19",
        "ShirtBuilder for Windows",
        &[
            ("new", "ShirtBuilder runs on Windows 10 and 11: an .msi installer adds it to the Start menu and opens .sb design files with ShirtBuilder; uninstall it from Settings › Apps."),
            ("improved", "The window and taskbar show the ShirtBuilder icon."),
            ("improved", "When no graphics driver can be used, the advice fits your system (Windows or Linux)."),
        ],
    ),
    (
        "0.0.14",
        "2026-11-19",
        "Drag, resize and edit text that works",
        &[
            ("fixed", "Resizing artwork from its corners and rotating it with the knob didn't work on the pattern: the drag only registered after the mouse had moved a few pixels, by when it had already left the small handle, so the view panned or the artwork moved instead. What you grab is now decided where you press the mouse button — and moving artwork follows the mouse exactly, without a jump."),
            ("improved", "Corner and rotate handles are easier to hit."),
            ("fixed", "Text layers had an unlabelled box at the top of their panel that only renamed the layer, so typing there didn't change the print. That box is now the labelled Text box with the printed words; the layer is named after them."),
            ("improved", "New text starts as LET'S BUILD IT."),
        ],
    ),
    (
        "0.0.13",
        "2026-11-19",
        "New, Open, Save — .sb design files",
        &[
            ("new", "New, Open and Save buttons in the top bar (Ctrl+N, Ctrl+O, Ctrl+S). Designs are saved as .sb files — the whole design, images included — and open again exactly as saved."),
            ("new", "Save writes straight into the file you're working on; the first time it asks where. A dot on the Save button means unsaved changes, and the window title shows the file name (• = unsaved)."),
            ("new", "Save as… (Ctrl+Shift+S, also in the Export menu) saves a copy under a new name."),
            ("new", "New and Open ask before unsaved changes would be lost: Save, Don't save or Cancel — and Ctrl+Z still brings the previous design back."),
            ("new", "ShirtBuilder remembers the file you were editing when you reopen it, and .sb files open in ShirtBuilder from the file manager (double-click) or by dropping them on the window."),
            ("improved", "Web app projects (.json) still open; the web .json export is in the Export menu as “Web app project”."),
            ("fixed", "Numbers in a design now read back exactly as saved."),
        ],
    ),
    (
        "0.0.12",
        "2026-11-19",
        "Top bar fits narrow windows",
        &[
            ("fixed", "Below about 1360 px wide the Sewing pattern / Split / 3D switcher slid over the help button and the garment name, so clicks landed on the wrong button. The top bar now never overlaps, down to the smallest window (960 px)."),
            ("improved", "As the window narrows the top bar tidies itself step by step: Theme, Export SVG and 3D snapshot become icon buttons (same tooltips), then the switcher reads Pattern and the logo drops its name, then the garment picker becomes a shirt icon. At the usual size nothing changes."),
        ],
    ),
    (
        "0.0.11",
        "2026-11-19",
        "White shirt, black text",
        &[
            ("fixed", "The white default didn't reach designs saved before v0.0.9: they reopened green with white text. Once, on the first start of this version, a design still on the old green becomes white, and white text on a white or light shirt — invisible there — becomes black. Colours you choose afterwards are never changed again."),
            ("improved", "Text starts black (#000000) on light shirts, including the default slogan; dark shirts still get white text."),
        ],
    ),
    (
        "0.0.10",
        "2026-11-19",
        "Changelog you can read to the end",
        &[
            ("fixed", "The What's new window grew taller than the app with every release, pushing its title and close button off screen so it couldn't be closed. It now always fits the window and the release list scrolls inside it."),
            ("improved", "What's new and Shortcuts open as dialogs over the dimmed app: close them with the × button, Esc, or a click outside."),
            ("improved", "Release notes are easier to scan: NEW / IMPROVED / FIXED tags in their own column and the latest release marked."),
            ("fixed", "The › in menu paths such as Scene › Graphics card showed as an empty box."),
        ],
    ),
    (
        "0.0.9",
        "2026-11-19",
        "White by default, roomier pattern sheet",
        &[
            ("improved", "New designs start on a white shirt, with the chest slogan printed in black so it shows."),
            ("improved", "The gradient's second colour defaults to a light grey that suits the white shirt."),
            ("fixed", "The sleeves' names and measurements no longer crowd the shoulders of the Front and Back pieces: the pattern sheet leaves 5 cm more room between the rows (also in exported sheets)."),
        ],
    ),
    (
        "0.0.8",
        "2026-11-19",
        "True-to-life body depth",
        &[
            ("fixed", "The realistic male and female tees looked thin from the side: their front-to-back depth was squashed to fit the simplified generated shirt. They now keep the models' real proportions (male 39% deeper, female 29% deeper, female 9% wider), matching the original 3D files."),
            ("improved", "Fold shadows are baked on the true shape, so they sit where the folds really are."),
        ],
    ),
    (
        "0.0.7",
        "2026-11-19",
        "Smoother male round-neck model",
        &[
            ("new", "The realistic male Round Neck T-Shirt now uses the new \"none wrinkle\" model: the same cut with smooth, freshly pressed fabric, so prints read cleanly."),
            ("fixed", "A few specks of the left-sleeve print no longer show on the front of the sleeve."),
        ],
    ),
    (
        "0.0.6",
        "2026-11-19",
        "Graphics card check",
        &[
            ("new", "When the 3D can't use the graphics card, the 3D view says so (click it for Scene › Graphics card) instead of just being slow."),
            ("new", "Spots an NVIDIA driver update that is waiting for a computer restart — the usual reason the card can't be used — and says to restart; after that ShirtBuilder picks the card by itself."),
        ],
    ),
    (
        "0.0.5",
        "2026-11-19",
        "In the app launcher",
        &[
            ("new", "ShirtBuilder shows up in the app launcher with its own icon (packaging/install-desktop.sh installs or updates it; --uninstall removes it)."),
        ],
    ),
    (
        "0.0.4",
        "2026-11-18",
        "3D preview, real prints, exports and themes",
        &[
            ("new", "3D preview on the graphics card: the realistic male and female tee models and the generated shirt for every style, ghost mannequin or on a hanger, with buttons, collars and rib."),
            ("new", "Studio lighting like the web app — fabric sheen, knit / weave / fleece texture, soft shadows, the window-light pattern, backdrops, exposure, brightness and contrast."),
            ("new", "Orbit, zoom and pan with the mouse; Front / Side / ¾ / Back views, auto-rotate, Reset view and a working navigation gizmo (click an axis, drag to orbit)."),
            ("new", "3D quality Auto / High / Balanced / Fast, with the frame rate shown on request; Auto steps down on slow computers."),
            ("new", "Real prints everywhere: the web app's fonts (Permanent Marker, Anton, Bebas Neue, Bangers, Inter, Space Grotesk, Playfair Display, Pacifico), bold and italic, alignment, line height and letter spacing."),
            ("new", "Image artwork: PNG, JPEG, WebP, GIF and SVG — from the Image button, the Image tool (I) or by dropping a picture onto the window; Replace image."),
            ("new", "Artwork continues across seams onto the neighbouring piece, in the pattern and in 3D."),
            ("new", "Scale and rotate handles on the selected artwork, snapping to the piece's centre line, and dragging artwork onto another piece moves it there."),
            ("new", "Duplicate layer (Ctrl+D), rectangle / ellipse shapes with rounded corners, centre and 0° buttons."),
            ("new", "Exports: print sheets (PNG 100 / 150 dpi, outside and inside), true-scale print & cut SVGs, cut lines only, and a 2000 px 3D snapshot — all with seam & hem allowances on or off."),
            ("new", "All 30 themes from the web app — standard, high contrast, Liquid Glass and Cyberpunk — with the theme gallery, Follow system and accent colours."),
            ("new", "Eyedropper: pick the fabric colour from anywhere in the window."),
        ],
    ),
    (
        "0.0.3",
        "2026-11-18",
        "Graphics card",
        &[
            ("new", "Scene › Graphics card: ShirtBuilder finds your graphics cards and draws the 3D (and the whole window) on the best one — a dedicated card before integrated graphics, the CPU only when nothing else works."),
            ("new", "Choose a specific card instead; the choice is saved and applies after a one-click restart."),
            ("new", "A clear warning when the app is drawing on the CPU, with the driver to install."),
        ],
    ),
    (
        "0.0.2",
        "2026-11-18",
        "Own window title bar",
        &[
            ("new", "The top bar is the window's title bar, like SolidCraft: minimize, maximize / restore and close buttons at the right, drag the bar to move the window, double-click it to maximize."),
            ("new", "Resize the window from any edge or corner."),
        ],
    ),
    (
    "0.0.1",
    "2026-11-18",
    "Rust edition — first release",
    &[
        ("new", "Native desktop app and web build from one Rust codebase (egui + wgpu)."),
        ("new", "Same layout as the web app: top bar, sewing-pattern pane, 3D pane and the Design / Scene sidebar."),
        ("new", "All garments and sizes, identical to the web app: 4 styles × short/long sleeve × male/female × XS–XXL (checked against it point by point)."),
        ("new", "Cut lines with seam & hem allowances, notches, grain lines, inside / outside view, rulers, grid, zoom and pan."),
        ("new", "Add, select, move, nudge, reorder, hide and delete artwork layers; edit position, size, rotation, opacity and text."),
        ("new", "Fabric colour, gradient, material and appearance settings; scene, lighting and display settings."),
        ("new", "Open and save project files — the same .json files as the web app, in both directions; autosave."),
        ("new", "Undo / redo and the web app's keyboard shortcuts."),
    ],
    ),
];
