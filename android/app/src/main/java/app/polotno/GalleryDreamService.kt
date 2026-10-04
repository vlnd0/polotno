package app.polotno

import android.service.dreams.DreamService

class GalleryDreamService : DreamService() {
    private var session: GallerySession? = null
    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        isInteractive = false // Let Android dismiss the dream and return to the previous application.
        isFullscreen = true
        isScreenBright = false
    }
    override fun onDreamingStarted() {
        super.onDreamingStarted()
        try { session = GallerySession(this, { }).also { setContentView(it.view) } }
        catch (_: Exception) { finish() }
    }
    override fun onDreamingStopped() { session?.close(); session = null; super.onDreamingStopped() }
    override fun onDetachedFromWindow() { session?.close(); session = null; super.onDetachedFromWindow() }
}
