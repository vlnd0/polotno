package app.polotno

import android.content.Context
import android.opengl.GLSurfaceView
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import org.json.JSONObject
import java.io.File

object EmbeddedServer {
    private var owners = 0
    private var info: JSONObject? = null
    fun acquire(context: Context): JSONObject {
        if (owners == 0) {
            // Only packaged editor assets are replaced. Gallery scenes and media are untouched.
            fun copyAsset(name: String) {
                val children = context.assets.list(name) ?: emptyArray()
                val file = File(context.filesDir,"gallery/$name")
                if (name.startsWith("editor/wallpapers/") && name != "editor/wallpapers/manifest.json" && file.exists()) return
                if (children.isNotEmpty()) { file.mkdirs(); children.forEach { copyAsset("$name/$it") } }
                else { file.parentFile?.mkdirs(); context.assets.open(name).use { input -> file.outputStream().use { input.copyTo(it) } } }
            }
            copyAsset("editor")
            info = JSONObject(NativeCore.start(File(context.filesDir, "gallery").absolutePath, 8787))
            NativeCore.configureMedia(context.applicationInfo.nativeLibraryDir)
        }
        owners += 1
        return requireNotNull(info)
    }
    fun pairing(context: Context): JSONObject = JSONObject(NativeCore.start(File(context.filesDir, "gallery").absolutePath, 0)).also { info = it }
    fun release() {
        check(owners > 0)
        owners -= 1
        if (owners == 0) { NativeCore.stop(); info = null }
    }
}

class GallerySession(context: Context, private val onError: (String) -> Unit, private val onControllerConnected: () -> Unit = {}) {
    val view = GLSurfaceView(context)
    val info: JSONObject = EmbeddedServer.acquire(context)
    private val handler = Handler(Looper.getMainLooper())
    private val renderer = GalleryRenderer(context, view, File(context.filesDir, "gallery/media"), onError)
    private val clock = PlaylistClock()
    private var snapshot: Snapshot? = null
    private var lastLayers: List<Layer>? = null
    private var closed = false
    private var controllerEpoch = 0L
    private val poll = object : Runnable {
        override fun run() {
            if (closed) return
            try {
                val fresh = if (snapshot?.revision != NativeCore.revision()) Snapshot.parse(NativeCore.snapshot()) else requireNotNull(snapshot)
                snapshot = fresh
                if (fresh.controllerEpoch > controllerEpoch) { controllerEpoch = fresh.controllerEpoch; onControllerConnected() }
                val items = clock.items(fresh, SystemClock.elapsedRealtime())
                val layers = fresh.regions.map { r -> Layer(r.id, r.corners, if(r.calibration)null else items[r.id]?.let { fresh.assets[it] }, r.muted,r.frame,r.fit,r.calibration,r.flipHorizontal,r.flipVertical) }
                if (layers != lastLayers) { renderer.submit(layers); lastLayers = layers }
                renderer.setPaused(fresh.paused)
            } catch (_: Exception) { onError("Не удалось обновить сцену") }
            handler.postDelayed(this, 80)
        }
    }
    init {
        view.setEGLContextClientVersion(2)
        view.preserveEGLContextOnPause = true
        view.setRenderer(renderer)
        view.renderMode = GLSurfaceView.RENDERMODE_WHEN_DIRTY
        handler.post(poll)
    }
    fun close() {
        if (closed) return
        closed = true
        handler.removeCallbacks(poll)
        renderer.close()
        view.onPause()
        EmbeddedServer.release()
    }
}
