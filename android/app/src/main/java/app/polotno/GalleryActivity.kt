package app.polotno

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.Color
import android.net.ConnectivityManager
import android.os.Bundle
import android.os.Debug
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.WindowManager
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.FrameLayout
import com.google.zxing.BarcodeFormat
import com.google.zxing.qrcode.QRCodeWriter
import java.net.Inet4Address

class GalleryActivity : Activity() {
    private var session: GallerySession? = null
    private lateinit var container: FrameLayout
    private var pairing: LinearLayout? = null
    private val handler = Handler(Looper.getMainLooper())
    private var lastAddress: String? = null
    private var ticks = 0
    private val networkPoll = object : Runnable {
        override fun run() {
            val address = localAddress()
            if (address != lastAddress) { lastAddress = address; if (pairing != null) showPairing() }
            if (++ticks % 15 == 0) Log.i("Polotno", "pssKb=${Debug.getPss()}")
            handler.postDelayed(this,2000)
        }
    }
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        container = FrameLayout(this); setContentView(container)
        window.decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_FULLSCREEN or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
        // Application window flag only; do not change system sleep, brightness or power settings.
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    }
    override fun onResume() {
        super.onResume()
        try {
            val current = GallerySession(this,::showError,::hidePairing); session = current
            container.addView(current.view,0,FrameLayout.LayoutParams(-1,-1))
            lastAddress = localAddress(); if (!current.info.optBoolean("has_paired_clients",false)) showPairing(); handler.post(networkPoll)
        } catch (_: Exception) { showError("Не удалось открыть Полотно") }
    }
    override fun onPause() {
        handler.removeCallbacks(networkPoll)
        session?.let { it.close(); container.removeView(it.view) }; session = null
        pairing?.let { container.removeView(it) }; pairing = null
        super.onPause()
    }
    private fun localAddress(): String? {
        val manager = getSystemService(ConnectivityManager::class.java)
        return manager.activeNetwork?.let { manager.getLinkProperties(it) }?.linkAddresses?.map { it.address }?.filterIsInstance<Inet4Address>()?.firstOrNull { !it.isLoopbackAddress }?.hostAddress
    }
    private fun showPairing() {
        if (session == null) return
        pairing?.let { container.removeView(it) }
        val panel = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; gravity = Gravity.CENTER; setPadding(28,28,28,28); setBackgroundColor(Color.rgb(20,23,29)) }
        val info = EmbeddedServer.pairing(this)
        val address = localAddress()
        val title = TextView(this).apply { setText(R.string.pairing_title); textSize = 22f; setTextColor(Color.WHITE); gravity = Gravity.CENTER }
        panel.addView(title)
        if (address != null) {
            val base = "http://$address:${info.getInt("port")}/"
            val url = "$base#pair=${info.getString("pairing_code")}" // Fragment is never sent as an HTTP URL.
            val bits = QRCodeWriter().encode(url,BarcodeFormat.QR_CODE,300,300)
            val bitmap = Bitmap.createBitmap(300,300,Bitmap.Config.ARGB_8888)
            for (y in 0 until 300) for (x in 0 until 300) bitmap.setPixel(x,y,if(bits[x,y])Color.BLACK else Color.WHITE)
            panel.addView(ImageView(this).apply { setImageBitmap(bitmap) },LinearLayout.LayoutParams(300,300))
            panel.addView(TextView(this).apply { text = getString(R.string.pairing_code,info.getString("display_code")); textSize = 24f; setTextColor(Color.WHITE); gravity = Gravity.CENTER })
            panel.addView(TextView(this).apply { text = getString(R.string.pairing_help,base); setTextColor(Color.WHITE); gravity = Gravity.CENTER })
        } else panel.addView(TextView(this).apply { setText(R.string.network_missing); setTextColor(Color.WHITE) })
        pairing = panel
        container.addView(panel,FrameLayout.LayoutParams(460,-2,Gravity.CENTER))
    }
    private fun hidePairing() { pairing?.let { container.removeView(it) }; pairing = null }
    private fun showError(message: String) { android.widget.Toast.makeText(this,message,android.widget.Toast.LENGTH_LONG).show() }
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (event.repeatCount > 0) return super.onKeyDown(keyCode,event)
        val command = when(keyCode) {
            KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_UP -> "previous"
            KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_DPAD_DOWN -> "next"
            KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER -> "pause"
            KeyEvent.KEYCODE_MENU -> { if (pairing == null) showPairing() else { container.removeView(pairing); pairing = null }; return true }
            else -> return super.onKeyDown(keyCode,event)
        }
        if (session != null) try { NativeCore.command(command) } catch (_: Exception) { showError("Не удалось выполнить команду") }
        return true
    }
}
