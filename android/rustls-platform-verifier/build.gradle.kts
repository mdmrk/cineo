plugins {
    id("com.android.library")
}

android {
    namespace = "org.rustls.platformverifier"
    compileSdk {
        version = release(37) { minorApiLevel = 2 }
    }

    defaultConfig {
        minSdk = 26
        buildConfigField("boolean", "TEST", "false")
    }

    buildFeatures {
        buildConfig = true
    }
}
