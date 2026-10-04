package app.polotno

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.os.Build
import org.json.JSONObject
import java.io.File
import kotlin.math.min

object NativeMedia {
    @JvmStatic fun decodeImage(source:String,target:String):String {
        return try {
            val bitmap=if(Build.VERSION.SDK_INT>=28)ImageDecoder.decodeBitmap(ImageDecoder.createSource(File(source))){decoder,info,_->
                if(info.isAnimated)throw IllegalArgumentException("Animated image uses video importer")
                decoder.allocator=ImageDecoder.ALLOCATOR_SOFTWARE
                val ratio=min(1f,2048f/maxOf(info.size.width,info.size.height))
                decoder.setTargetSize((info.size.width*ratio).toInt().coerceAtLeast(1),(info.size.height*ratio).toInt().coerceAtLeast(1))
            }else BitmapFactory.decodeFile(source)?:return ""
            try { File(target).outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG,100,it) };JSONObject().put("width",bitmap.width).put("height",bitmap.height).toString() }finally{bitmap.recycle()}
        }catch(_:Exception){""}
    }
}
