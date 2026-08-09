# Private Android JNI application gate

This directory is a dependency-free, no-UI Android packaging fixture for the
private SEMAPRAX JNI ownership boundary. It is not a public Android SDK or an
AAR project.

The Gradle build applies no plugin and declares no repository. Its single task
runs `package.sh`, which requires runner-provided Kotlin 2 and the exact Android
35 build tools, consumes exactly three generated x86_64 shared libraries, and
assembles one same-package framework `Instrumentation` APK. CI invokes Gradle
with `--offline`.

`scripts/android-jni-app-v3.sh` owns the complete hosted flow: generate the
target-bound native fixtures, compile x86_64 and arm64 evidence with pinned NDK
r27.2, inspect the ELF files, build and inspect the APK, install it into the
pinned API-35 x86_64 Emulator, run the instrumentation, and exact-match the
app-private result file.

The Kotlin wrapper uses an API-28-compatible `PhantomReference`/
`ReferenceQueue` cleanup path. Automatic cleanup only enqueues consumption to
the runtime's owning `HandlerThread`; the Cleaner thread never enters the
thread-confined native host.

This fixture does not claim UI behavior, arm64 device execution, AAR
publication, broad Android lifecycle integration, or public native admission.
