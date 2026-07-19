plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "io.github.lamggm.auxscreen"
    compileSdk = 36

    defaultConfig {
        applicationId = "io.github.lamggm.auxscreen"
        minSdk = 30
        targetSdk = 36
        versionCode = 2
        versionName = "0.1.0-rc.1"
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        buildConfigField("boolean", "ALLOW_LAN_CLEARTEXT", "false")

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    val personalKeystore = providers.environmentVariable("AUXSCREEN_KEYSTORE").orNull
    val personalStorePassword = providers.environmentVariable("AUXSCREEN_STORE_PASSWORD").orNull
    val personalKeyAlias = providers.environmentVariable("AUXSCREEN_KEY_ALIAS").orNull
    val personalKeyPassword = providers.environmentVariable("AUXSCREEN_KEY_PASSWORD").orNull
    val hasPersonalSigning = listOf(
        personalKeystore,
        personalStorePassword,
        personalKeyAlias,
        personalKeyPassword,
    ).all { !it.isNullOrBlank() }

    signingConfigs {
        if (hasPersonalSigning) {
            create("personal") {
                storeFile = file(personalKeystore!!)
                storePassword = personalStorePassword
                keyAlias = personalKeyAlias
                keyPassword = personalKeyPassword
            }
        }
    }

    buildTypes {
        debug {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            buildConfigField("boolean", "ALLOW_LAN_CLEARTEXT", "true")
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
        create("personal") {
            initWith(getByName("release"))
            // R8 triggers a native JNI_OnLoad SIGTRAP in libwebrtc on SM-X400/API 36.
            // Keep this private-LAN build non-debuggable but unshrunk until requalified.
            isMinifyEnabled = false
            isShrinkResources = false
            applicationIdSuffix = ".personal"
            versionNameSuffix = "-personal"
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            buildConfigField("boolean", "ALLOW_LAN_CLEARTEXT", "true")
            signingConfig = if (hasPersonalSigning) {
                signingConfigs.getByName("personal")
            } else {
                signingConfigs.getByName("debug")
            }
            matchingFallbacks += listOf("release")
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    packaging {
        resources.excludes += setOf("/META-INF/{AL2.0,LGPL2.1}")
    }

    testOptions {
        unitTests.isIncludeAndroidResources = true
    }

    sourceSets.getByName("test").resources.directories.add("../../protocol/fixtures")
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2026.06.00")
    implementation(composeBom)
    androidTestImplementation(composeBom)

    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.10.0")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("io.github.webrtc-sdk:android:144.7559.09")
    implementation("com.squareup.okhttp3:okhttp:5.1.0")

    debugImplementation("androidx.compose.ui:ui-tooling")
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20250517")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
