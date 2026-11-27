import javax.inject.Inject
import org.gradle.process.ExecOperations

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val cargoWorkspace: Directory = rootProject.layout.projectDirectory.dir("..")
val minApi: Int by rootProject.extra

fun cargoLockVersion(crate: String): String {
    val lines = cargoWorkspace.file("Cargo.lock").asFile.readLines()
    val at = lines.indexOfFirst { it.trim() == "name = \"$crate\"" }
    require(at >= 0) { "$crate is not in Cargo.lock" }
    return lines.drop(at + 1).first { it.startsWith("version = ") }
        .substringAfter('"').substringBefore('"')
}

fun cargoTomlValue(section: String, key: String): String {
    val lines = cargoWorkspace.file("Cargo.toml").asFile.readLines()
    val at = lines.indexOfFirst { it.trim() == "[$section]" }
    require(at >= 0) { "Cargo.toml has no [$section]" }
    val line = lines.drop(at + 1).takeWhile { !it.startsWith("[") }.firstOrNull { it.startsWith("$key = ") }
    return requireNotNull(line) { "Cargo.toml [$section] has no $key" }.substringAfter("= ").trim().trim('"')
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
            environment("CARGO_PROFILE_RELEASE_PANIC", "unwind")
            val home = System.getProperty("user.home")
            val remaps = listOf(
                (System.getenv("CARGO_HOME") ?: "$home/.cargo") to "/cargo",
                (System.getenv("RUSTUP_HOME") ?: "$home/.rustup") to "/rustup",
                workspace.get().asFile.path to "/cineo",
            ).map { (from, to) -> "--remap-path-prefix=${File(from).canonicalPath}=$to" }
            environment("CARGO_ENCODED_RUSTFLAGS", remaps.joinToString("\u001f"))
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
                "cargo", "run", "-p", "cineo-uniffi-bindgen", "--bin", "uniffi-bindgen", "--locked", "--",
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

abstract class Libmpv : DefaultTask() {
    @get:InputFiles @get:PathSensitive(PathSensitivity.RELATIVE) abstract val built: DirectoryProperty
    @get:Input abstract val required: Property<Boolean>
    @get:OutputDirectory abstract val jniLibs: DirectoryProperty
    @get:OutputDirectory abstract val assets: DirectoryProperty

    @TaskAction
    fun copy() {
        val out = built.get().asFile
        val abi = jniLibs.get().dir("arm64-v8a").asFile
        val licenses = assets.get().dir("licenses").asFile
        listOf(abi, licenses).forEach { it.deleteRecursively(); it.mkdirs() }
        val libs = out.resolve("lib").listFiles { file -> file.name.endsWith(".so") }.orEmpty()
        val notice = out.resolve("libmpv.md")
        libs.forEach { it.copyTo(abi.resolve(it.name)) }
        out.resolve("licenses").listFiles().orEmpty().forEach { it.copyTo(licenses.resolve(it.name)) }
        if (notice.isFile) notice.copyTo(licenses.resolve(notice.name))
        if (libs.none { it.name == "libmpv.so" } || !notice.isFile) {
            val message = "No libmpv build at $out. Run scripts/build-libmpv-android.sh."
            if (required.get()) throw GradleException(message)
            logger.warn("$message The app will build, but cannot play.")
        }
    }
}

fun versionCode(version: String): Int {
    val parts = Regex("""(\d+)\.(\d+)\.(\d+)""").matchEntire(version)?.destructured?.toList()?.map { it.toInt() }
    requireNotNull(parts) { "the workspace version $version is not MAJOR.MINOR.PATCH" }
    val (major, minor, patch) = parts
    require(minor < 100 && patch < 100) { "the versionCode needs MINOR and PATCH below 100, not $version" }
    val expected = major * 10000 + minor * 100 + patch
    val declared = requireNotNull(cargoTomlValue("workspace.metadata.android", "version-code").toIntOrNull()) {
        "Cargo.toml [workspace.metadata.android] version-code is not an integer"
    }
    require(declared == expected) {
        "Cargo.toml [workspace.metadata.android] version-code is $declared, but version $version needs $expected"
    }
    return declared
}

fun env(name: String): String? = providers.environmentVariable(name).orNull?.takeIf { it.isNotEmpty() }

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
    ndkVersion = rootProject.extra["ndkVersion"] as String

    defaultConfig {
        applicationId = "io.github.mdmrk.cineo"
        minSdk = minApi
        targetSdk = 36
        versionName = cargoTomlValue("workspace.package", "version")
        versionCode = versionCode(cargoTomlValue("workspace.package", "version"))
        ndk { abiFilters += "arm64-v8a" }
    }

    signingConfigs {
        create("release") {
            env("CINEO_KEYSTORE")?.let { storeFile = file(it) }
            storePassword = env("CINEO_KEYSTORE_PASSWORD")
            keyAlias = env("CINEO_KEY_ALIAS")
            keyPassword = env("CINEO_KEY_PASSWORD")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            if (env("CINEO_KEYSTORE") != null) signingConfig = signingConfigs.getByName("release")
        }
    }

    packaging {
        jniLibs {
            useLegacyPackaging = true
            keepDebugSymbols += listOf("**/libmpv.so", "**/libav*.so", "**/libsw*.so")
        }
    }

    buildFeatures {
        compose = true
    }

    dependenciesInfo {
        includeInApk = false
    }
}

composeCompiler {
    stabilityConfigurationFiles.add(layout.projectDirectory.file("compose-stability.conf"))
}

androidComponents {
    onVariants { variant ->
        val libmpv = tasks.register<Libmpv>("libmpv${variant.name.replaceFirstChar { it.uppercase() }}") {
            built.set(cargoWorkspace.dir("target/libmpv-android/out"))
            required.set(variant.buildType == "release")
            jniLibs.set(layout.buildDirectory.dir("generated/libmpv/${variant.name}/jniLibs"))
            assets.set(layout.buildDirectory.dir("generated/libmpv/${variant.name}/assets"))
        }
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargoNdk, CargoNdk::jniLibs)
        variant.sources.jniLibs?.addGeneratedSourceDirectory(libmpv, Libmpv::jniLibs)
        variant.sources.assets?.addGeneratedSourceDirectory(libmpv, Libmpv::assets)
        variant.sources.kotlin?.addGeneratedSourceDirectory(uniffiBindgen, UniffiBindgen::kotlin)
    }
}

run {
    val vendored = rootProject.layout.projectDirectory.file("rustls-platform-verifier/UPSTREAM").asFile
        .readLines().first().substringAfterLast(' ')
    val locked = cargoLockVersion("rustls-platform-verifier-android")
    require(vendored == locked) {
        "Cargo.lock has rustls-platform-verifier-android $locked, but android/rustls-platform-verifier is $vendored"
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2026.11.01"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.camera:camera-camera2:1.7.0")
    implementation("androidx.camera:camera-lifecycle:1.7.0")
    implementation("androidx.camera:camera-view:1.7.0")
    implementation("com.google.zxing:core:3.5.4")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.1")
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
    implementation(project(":rustls-platform-verifier"))
    testImplementation("junit:junit:4.13.2")
}

tasks.withType<Test>().configureEach {
    inputs.dir("src/main/res")
}
