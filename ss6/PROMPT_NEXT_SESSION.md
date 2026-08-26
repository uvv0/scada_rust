# Next Session Prompt

Work area:
- desktop/editor: `C:\andr\my2\ss7`
- web UI: `C:\andr\my2\ss6`

State date: 2026-05-05.

## Current State

`ss7` desktop/editor:
- Added `[IMAGE]` layout item via `Add image`.
- Image item stores path in `label_override` / `image_path`.
- `fmt` is image fit mode: `contain`, `cover`, `stretch`.
- `scale_max` is image opacity `0.0..1.0`.
- Preview loads PNG/JPG with the Rust `image` crate.
- Preview caches image textures to avoid flicker.
- Image items are drawn first, as a background layer, so they do not cover parameters or labels.
- Web-safe analysis now matches `ss6` left-label behavior for narrow KP4 `tit_ustavki` min/max cells.
- Web-safe warnings now catch bad image paths: empty, absolute Windows path, `..`, and too-small image tile.

Fresh `ss7.exe`:
- `C:\andr\my2\ss7\target\release\ss7.exe`
- size: `4 530 176`
- time: `2026-05-05 16:01:09`

`ss6` web UI:
- `/api/ui_bindings` returns image items from `ui.kpz_window_text_item` as `component_kind='image'`.
- Added protected route `/ui_images/{*path}`.
- Route serves files only from `ui_images` next to `ss6.exe` working directory.
- Absolute paths and `..` are blocked.
- `assets/preview_scene.js` renders image tiles with `<img>`.
- CSS for `.tile--image`, `.tile__image`, `.tile__image-chip`, `.tile__image-placeholder` is in `src/web.rs`.
- Image tiles are background layer: lower z-index and inserted before other tiles.
- Left labels are above image/background.
- KP4 `tit_ustavki` fix: min/max labels disappeared because old web rule required `x>=90` and `w>=120`; KP4 cells are around `x=48/152`, `w=60`. Rule now shows external labels for `auto`, `numeric`, `setpoint`, `bar`, `gauge`, `trend` regardless of cell width.
- Left-label text has `maxWidth` and `title` tooltip to avoid visual overflow while keeping full text available.

Fresh `ss6.exe`:
- `C:\andr\my2\ss6\target\release\ss6.exe`
- size: `3 557 376`
- time: `2026-05-05 15:57:25`

Verified:
- `ss6 cargo check` OK.
- `ss6 cargo build --release` OK.
- `ss7 cargo check` OK.
- `ss7 cargo build --release` OK.

## How To Check KP4 Labels

KP4 window:
- `kpz_id=4`
- `ui.kpz_window.id=68`
- code `15`
- title is `tit_ustavki` in broken console encoding.

Important rows from DB:
- `min1`: `reg_id=40017`, `x=48`, `w=60`
- `max1`: `reg_id=40018`, `x=152`, `w=63`
- further min/max rows continue similarly.

Expected in web preview:
- labels like min1/max1 appear to the left of value tiles.
- image/background must not cover those labels.

## How To Check Technical Scheme Image

1. Create:
   `C:\andr\my2\ss6\ui_images`

2. Put image there, for example:
   `C:\andr\my2\ss6\ui_images\scheme.png`

3. In `ss7`, open UI Layout and click `Add image`.

4. In image path use:
   `scheme.png`
   or
   `ui_images/scheme.png`

5. Save with `Save all`.

6. Start `ss6.exe`, open web preview, select the same window.

Expected:
- `ss7` preview shows the image.
- `ss6` web preview shows the image.
- dynamic values and left labels are above the image.
- wrong path shows placeholder/error, not a blank crash.

## Next Useful Section

Recommended next work:
- Test KP4 `tit_ustavki` in browser after restarting `ss6.exe`.
- Test real PNG/JPG image in `ui_images`.
- If labels still differ between `ss7` and `ss6`, compare `web_safe_uses_external_label` in `ss7` with `useExternalLabel` in `ss6/assets/app.js`.
- Add a small UI hint near `[IMAGE]`: `Use ui_images/name.png for web`.
- Optionally add a debug chip in web preview showing: window id, binding count, text/image count, left-label count.

Important:
- Do not use absolute Windows paths in web image path.
- Use relative `ui_images/name.png` paths for web-safe mode.

