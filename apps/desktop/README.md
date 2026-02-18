# Cratos Native App (Tauri + Next.js + Tailwind v4)

This project is a native desktop (and mobile) application built with [Tauri v2](https://v2.tauri.app/), [Next.js v16](https://nextjs.org), and [Tailwind CSS v4](https://tailwindcss.com).

## 🚀 Getting Started

### Development
1. Install dependencies:
   ```bash
   npm install
   ```
2. Run development server (Next.js + Tauri):
   ```bash
   npm run tauri dev
   ```

### Building for Production (macOS)
```bash
npm run tauri build
```
The output `.app` will be located at:
`src-tauri/target/release/bundle/macos/Cratos.app`

---

## 📱 Mobile Setup Guide (Manual Steps Required)

### 1. iOS Configuration (Xcode)
The iOS project is generated at `src-tauri/gen/apple`.
To build for a physical device, you must configure **Code Signing**:
1. Open the project in Xcode:
   ```bash
   open src-tauri/gen/apple/appsdesktop.xcodeproj
   ```
2. Select the root project `appsdesktop` in the left navigator.
3. Select the target `appsdesktop_iOS`.
4. Go to the **"Signing & Capabilities"** tab.
5. Under **"Team"**, select your apple developer team (or "Personal Team").
6. Verify `Bundle Identifier` is unique if using a free account.

### 2. Android Configuration (Android Studio)
The Android project generation requires **Android NDK** and **CMake**.
1. Open **Android Studio**.
2. Go to **Settings** (or Preferences) -> **Languages & Frameworks** -> **Android SDK**.
3. Select the **"SDK Tools"** tab.
4. Check **"NDK (Side by side)"** and **"CMake"**.
5. Click **Apply** to install.
6. Ensure `ANDROID_HOME` is set in your environment (e.g., in `.zshrc`):
   ```bash
   export ANDROID_HOME=$HOME/Library/Android/sdk
   export PATH=$PATH:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools
   ```
7. Run initialization again if needed:
   ```bash
   npm run tauri android init
   ```

---

## ⚠️ Known Issues & TODOs

### DMG Bundling Disabled
The `create-dmg` utility caused errors in the automated build environment.
- **Current Status**: DMG bundling is disabled in `src-tauri/tauri.conf.json`. Only `.app` is generated.
- **To Enable**:
  1. Ensure `create-dmg` is installed and working: `brew install create-dmg`.
  2. Edit `src-tauri/tauri.conf.json`:
     ```json
     "bundle": {
       "targets": ["app", "dmg", "updater"]
     }
     ```
