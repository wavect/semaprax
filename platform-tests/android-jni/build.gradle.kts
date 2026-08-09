import org.gradle.api.tasks.Exec

// Deliberately no Android/Kotlin/community plugin and no repository. The
// checked-in packaging script uses only runner-provided Kotlin and Android SDK
// tools. `--offline` is mandatory in CI and source-locked by Rust tests.
tasks.register<Exec>("assembleAndroidJni") {
    group = "verification"
    description = "Build the private framework-only Android JNI evidence APK"
    workingDir = projectDir
    commandLine("bash", "package.sh")
}
tasks.register("check") {
    group = "verification"
    dependsOn("assembleAndroidJni")
}
