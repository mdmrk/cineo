import javax.inject.Inject
import org.gradle.process.ExecOperations

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val cargoWorkspace: Directory = rootProject.layout.projectDirectory.dir("..")
val minApi = 26

fun cargoLockVersion(crate: String): String {
    val lines = cargoWorkspace.file("Cargo.lock").asFile.readLines()
    val at = lines.indexOfFirst { it.trim() == "name = \"$crate\"" }
    require(at >= 0) { "$crate is not in Cargo.lock" }
    return lines.drop(at + 1).first { it.startsWith("version = ") }
        .substringAfter('"').substringBefore('"')
}

fun workspaceVersion(): String {
    val lines = cargoWorkspace.file("Cargo.toml").asFile.readLines()
    val section = lines.indexOfFirst { it.trim() == "[workspace.package]" }
    return lines.drop(section + 1).first { it.startsWith("version = ") }
        .substringAfter('"').substringBefore('"')
}

abstract class CargoNdk : DefaultTask() {
    @get:Inject abstract val exec: ExecOperations
    @get:Internal abstract val workspace: DirectoryProperty
    @get:Internal abstract val ndk: DirectoryProperty
    @get:Input abstract val api: Property<Int>
    @get:OutputDirectory abstract val jniLibs: DirectoryProperty

    @TaskAction
    fun build() {
        exec.exec {
            workingDir = workspace.get().asFile
            environment("ANDROID_NDK_HOME", ndk.get().asFile.path)
            environment("CARGO_PROFILE_RELEASE_STRIP", "debuginfo")
            commandLine(
                "cargo", "ndk", "-t", "arm64-v8a", "-P", api.get().toString(),
                "-o", jniLibs.get().asFile.path,
                "build", "-p", "cineo-android", "--release", "--locked",
            )
        }
    }
}

abstract class UniffiBindgen : DefaultTask() {
    @get:Inject abstract val exec: ExecOperations
    @get:Internal abstract val workspace: DirectoryProperty
    @get:InputFiles abstract val jniLibs: DirectoryProperty
    @get:OutputDirectory abstract val kotlin: DirectoryProperty

    @TaskAction
    fun generate() {
        val library = jniLibs.get().file("arm64-v8a/libcineo_android.so").asFile
        exec.exec {
            workingDir = workspace.get().asFile
            commandLine(
                "cargo", "run", "-p", "cineo-android", "--features", "bindgen",
                "--bin", "uniffi-bindgen", "--locked", "--",
                "generate", "--library", library.path, "--language", "kotlin",
                "--no-format", "--out-dir", kotlin.get().asFile.path,
            )
        }
    }
}

val cargoNdk = tasks.register<CargoNdk>("cargoNdk") {
    workspace.set(cargoWorkspace)
    ndk.set(androidComponents.sdkComponents.ndkDirectory)
    api.set(minApi)
    jniLibs.set(layout.buildDirectory.dir("generated/rust/jniLibs"))
    outputs.upToDateWhen { false }
}

abstract class LibmpvLibs : DefaultTask() {
    @get:Internal abstract val built: DirectoryProperty
    @get:OutputDirectory abstract val jniLibs: DirectoryProperty

    @TaskAction
    fun copy() {
        val abi = jniLibs.get().dir("arm64-v8a").asFile
        abi.deleteRecursively()
        abi.mkdirs()
        val libs = built.get().asFile.listFiles { file -> file.name.endsWith(".so") }.orEmpty()
        if (libs.isEmpty()) {
            logger.warn("No libmpv at ${built.get().asFile}: the app will build, but cannot play. Run scripts/build-libmpv-android.sh.")
        }
        libs.forEach { it.copyTo(abi.resolve(it.name)) }
    }
}

val libmpvLibs = tasks.register<LibmpvLibs>("libmpvLibs") {
    built.set(cargoWorkspace.dir("target/libmpv-android/out/lib"))
    jniLibs.set(layout.buildDirectory.dir("generated/libmpv/jniLibs"))
    outputs.upToDateWhen { false }
}

val uniffiBindgen = tasks.register<UniffiBindgen>("uniffiBindgen") {
    workspace.set(cargoWorkspace)
    jniLibs.set(cargoNdk.flatMap { it.jniLibs })
    kotlin.set(layout.buildDirectory.dir("generated/rust/kotlin"))
}

android {
    namespace = "io.github.mdmrk.cineo"
    compileSdk {
        version = release(37) { minorApiLevel = 2 }
    }
    ndkVersion = "30.0.16248370"

    defaultConfig {
        applicationId = "io.github.mdmrk.cineo"
        minSdk = minApi
        targetSdk = 36
        versionCode = 1
        versionName = workspaceVersion()
        ndk { abiFilters += "arm64-v8a" }
    }

    buildFeatures {
        compose = true
    }
}

androidComponents {
    onVariants { variant ->
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargoNdk, CargoNdk::jniLibs)
        variant.sources.jniLibs?.addGeneratedSourceDirectory(libmpvLibs, LibmpvLibs::jniLibs)
        variant.sources.kotlin?.addGeneratedSourceDirectory(uniffiBindgen, UniffiBindgen::kotlin)
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2026.09.00"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
    implementation("org.rustls:rustls-platform-verifier:${cargoLockVersion("rustls-platform-verifier-android")}")
}
