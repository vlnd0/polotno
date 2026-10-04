plugins { id("com.android.application"); id("org.jetbrains.kotlin.android") }

android {
    namespace = "app.polotno"
    compileSdk = 35
    ndkVersion = "27.0.12077973"
    defaultConfig {
        applicationId = "app.polotno"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0-prototype"
        ndk { abiFilters += listOf("armeabi-v7a", "arm64-v8a", "x86_64") }
    }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    kotlinOptions { jvmTarget = "17" }
    sourceSets["main"].jniLibs.srcDir(layout.buildDirectory.dir("rustJniLibs"))
    sourceSets["main"].jniLibs.srcDir(layout.buildDirectory.dir("mediaJniLibs"))
    sourceSets["main"].assets.srcDir(layout.buildDirectory.dir("webAssets"))
    sourceSets["main"].assets.srcDir(layout.buildDirectory.dir("licenseAssets"))
    sourceSets["main"].assets.srcDir(layout.buildDirectory.dir("mediaLicenseAssets"))
    buildTypes { release { isMinifyEnabled = false } }
    packaging { jniLibs { useLegacyPackaging = true } }
}

val buildWeb by tasks.registering(Exec::class) {
    workingDir = rootProject.file("web")
    commandLine("npm", "run", "build")
    inputs.files(rootProject.fileTree("web/src"), rootProject.fileTree("web/public"), rootProject.file("web/index.html"), rootProject.file("web/package.json"), rootProject.file("web/package-lock.json"), rootProject.file("web/tsconfig.json"))
    outputs.dir(rootProject.file("web/dist"))
}
val packageWeb by tasks.registering(Sync::class) {
    dependsOn(buildWeb)
    from(rootProject.file("web/dist"))
    into(layout.buildDirectory.dir("webAssets/editor"))
}
val buildMediaTools by tasks.registering(Exec::class) {
    workingDir=rootProject.projectDir
    commandLine("python3","scripts/build-media-tools.py")
    inputs.file(rootProject.file("scripts/build-media-tools.py"))
    outputs.dir(layout.buildDirectory.dir("mediaJniLibs"))
    outputs.dir(layout.buildDirectory.dir("mediaLicenseAssets"))
}
val buildRust by tasks.registering(Exec::class) {
    workingDir = rootProject.projectDir
    commandLine("bash", "scripts/build-native.sh")
    inputs.files(rootProject.fileTree("core/src"), rootProject.fileTree("core/catalog"), rootProject.file("web/public/wallpapers/manifest.json"), rootProject.file("core/Cargo.toml"), rootProject.file("Cargo.toml"), rootProject.file("Cargo.lock"), rootProject.file("scripts/build-native.sh"), rootProject.file("scripts/bundle-rust-licenses.py"))
    outputs.dir(layout.buildDirectory.dir("rustJniLibs"))
    outputs.dir(layout.buildDirectory.dir("licenseAssets"))
}
tasks.named("preBuild") { dependsOn(buildRust, packageWeb, buildMediaTools) }

dependencies {
    implementation("androidx.media3:media3-exoplayer:1.6.1")
    implementation("com.google.zxing:core:3.5.3")
    testImplementation("junit:junit:4.13.2")
}
