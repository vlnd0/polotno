package app.polotno

object NativeCore {
    init { System.loadLibrary("polotno_core") }
    @JvmStatic external fun start(path: String, port: Int): String
    @JvmStatic external fun snapshot(): String
    @JvmStatic external fun revision(): Long
    @JvmStatic external fun command(command: String)
    @JvmStatic external fun stop()
    @JvmStatic external fun configureMedia(directory:String)
}
