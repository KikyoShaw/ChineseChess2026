# Third-party notices

## Bundled visual and audio resources

Source: ChineseChess. License: Apache License, Version 2.0. The original license text is preserved in `LICENSE`.

- `public/assets/images/`: wooden board, piece GIFs, selection markers and icons.
- `public/assets/audios/`: nine game sound effects.
- `src-tauri/icons/`: PNG/ICO conversions of the bundled red king image.
- Piece-square evaluation data is adapted to the application's independent Rust search implementation.

The application source is implemented independently with Tauri 2, Vue 3 and Rust. All bundled resources reside inside this project; no source checkout or installation of the resource provider is required at runtime.

## Dependencies

Tauri, Vue, Vite, Rust crates and npm packages retain their respective licenses, available in the installed dependency sources and upstream distributions.
