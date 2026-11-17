plugins {
    id("com.android.library")
}

android {
    namespace = "org.rustls.platformverifier"
    compileSdk {
        version = release(37) { minorApiLevel = 2 }
    }

    defaultConfig {
        minSdk = rootProject.extra["minApi"] as Int
        buildConfigField("boolean", "TEST", "false")
    }

    buildFeatures {
        buildConfig = true
    }
}
