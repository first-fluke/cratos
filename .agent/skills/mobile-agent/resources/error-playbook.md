# Mobile Agent - Error Playbook

## 1. Gradle/Build Errors (Android)
**Symptom**: `Task 'assembleDebug' failed` or dependency resolution error.
**Solution**:
- Check `src-tauri/gen/android/build.gradle.kts`.
- Run `cd src-tauri/gen/android && ./gradlew clean`.
- Verify standard library versions (Kotlin, SDK tools).

## 2. Xcode Signing Errors (iOS)
**Symptom**: `Provisioning profile "..." doesn't include the currently selected device`.
**Solution**:
- Open `src-tauri/gen/apple/Runner.xcodeproj` in Xcode.
- Check "Signing & Capabilities" tab.
- Ensure correct Team and Bundle Identifier selected.

## 3. Plugin Command Not Found
**Symptom**: `Command '...' not allowed by strict capability`.
**Solution**:
- Check `src-tauri/capabilities/mobile.json` (or `default.json`).
- Ensure the plugin is listed in `permissions`:
  ```json
  "permissions": [
    "core:default",
    "camera:default"
  ]
  ```
- Rebuild app (capabilities are baked in at build time).

## 4. White Screen on Launch
**Symptom**: App launches but stays white/blank.
**Solution**:
- Check output console for JS errors ("Inspect Element" via Safari/Chrome DevTools).
- Verify `next.config.js` allows `out` export or proper static generation if using SSG.
- Ensure the frontend dev server is reachable (if using dev mode) or built assets are correct.
