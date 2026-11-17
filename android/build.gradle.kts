plugins {
    id("com.android.application") version "9.5.0" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21" apply false
}

fun libmpvAndroidEnv(key: String): String =
    file("../scripts/libmpv-android.env").readLines().first { it.startsWith("$key=") }.substringAfter('=')

val minApi by extra(libmpvAndroidEnv("ANDROID_API").toInt())
val ndkVersion by extra(libmpvAndroidEnv("NDK_VERSION"))
