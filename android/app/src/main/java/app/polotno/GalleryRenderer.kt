package app.polotno

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.BitmapFactory
import android.graphics.SurfaceTexture
import android.opengl.GLES11Ext
import android.opengl.GLES20.*
import android.opengl.GLSurfaceView
import android.opengl.GLUtils
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Surface
import android.view.Choreographer
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.analytics.AnalyticsListener
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.atomic.AtomicBoolean
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.opengles.GL10
import kotlin.math.sqrt

data class Layer(val id: String, val corners: List<Point>, val asset: Asset?, val muted: Boolean, val frame: NeonFrame? = null, val fit:String = "contain",val calibration:Boolean=false,val flipHorizontal:Boolean=false,val flipVertical:Boolean=false)

@SuppressLint("UnsafeOptInUsageError")
class GalleryRenderer(private val context: Context, private val view: GLSurfaceView, private val media: File, private val onError: (String) -> Unit) : GLSurfaceView.Renderer {
    private val main = Handler(Looper.getMainLooper())
    private val vertices = ByteBuffer.allocateDirect(12 * 4).order(ByteOrder.nativeOrder()).asFloatBuffer().apply { put(floatArrayOf(0f,0f, 1f,0f, 0f,1f, 0f,1f, 1f,0f, 1f,1f)); position(0) }
    private var imageProgram = 0
    private var videoProgram = 0
    private var frameProgram = 0
    private var gridProgram = 0
    private var viewportWidth = 1920
    private var viewportHeight = 1080
    private var layers = emptyList<Layer>()
    private val images = mutableMapOf<String, Int>()
    private val imageAspects = mutableMapOf<String,Float>()
    @Volatile private var videoAspects = emptyMap<String,Float>()
    private data class Video(val texture: Int, val stream: SurfaceTexture, val surface: Surface, val pending: AtomicBoolean = AtomicBoolean(), var ready: Boolean = false, val transform: FloatArray = FloatArray(16))
    private val videos = mutableMapOf<String, Video>() // GL thread only.
    private data class Playback(val player: ExoPlayer, var assetId: String)
    private val players = mutableMapOf<String, Playback>() // Main thread only.
    @Volatile private var closed = false
    @Volatile private var paused = false
    @Volatile private var desired = emptyList<Layer>()
    @Volatile private var contextEpoch = 0L
    private var draws = 0L
    private var slowDraws = 0L
    private var totalDrawNanos = 0L
    private var frameScheduled=false // Main thread only.
    private var lastVideoFrame=0L
    private val videoFrame=object:Choreographer.FrameCallback {
        override fun doFrame(now:Long) {
            if(closed){frameScheduled=false;return}
            // Video imports run at <=30 fps. Coalesce all decoder callbacks into one scene draw.
            if(now-lastVideoFrame<32_000_000L){Choreographer.getInstance().postFrameCallback(this);return}
            lastVideoFrame=now;frameScheduled=false;view.requestRender()
        }
    }
    private fun requestVideoFrame(){
        if(!closed&&!frameScheduled){frameScheduled=true;Choreographer.getInstance().postFrameCallback(videoFrame)}
    }

    fun submit(value: List<Layer>) {
        desired = value
        view.queueEvent { if (!closed && imageProgram != 0) reconcile(value) }
        view.requestRender()
    }
    fun setPaused(value: Boolean) {
        if (paused == value) return
        paused = value
        players.values.forEach { it.player.playWhenReady = !value }
    }
    override fun onSurfaceCreated(gl: GL10?, config: EGLConfig?) {
        contextEpoch += 1
        // Context loss invalidates GL names. Detach old decoders before allocating replacements.
        main.post { players.values.forEach { it.player.release() }; players.clear() }
        videos.values.forEach { it.surface.release(); it.stream.release() }; videos.clear(); images.clear()
        imageProgram = program(false); videoProgram = program(true); frameProgram = frameProgram();gridProgram=gridProgram()
        glClearColor(0f, 0f, 0f, 1f)
        glEnable(GL_BLEND)
        glBlendFunc(GL_ONE,GL_ONE_MINUS_SRC_ALPHA) // Android bitmaps contain premultiplied alpha.
        Log.i("Polotno", "graphics=${glGetString(GL_VERSION)} maxTexture=${IntArray(1).also { glGetIntegerv(GL_MAX_TEXTURE_SIZE,it,0) }[0]}")
        if (!closed) reconcile(desired)
    }
    override fun onSurfaceChanged(gl: GL10?, width: Int, height: Int) { viewportWidth=width;viewportHeight=height;glViewport(0,0,width,height) }
    private fun newTexture(target: Int): Int {
        val names = IntArray(1); glGenTextures(1,names,0); glBindTexture(target,names[0])
        glTexParameteri(target,GL_TEXTURE_MIN_FILTER,GL_LINEAR); glTexParameteri(target,GL_TEXTURE_MAG_FILTER,GL_LINEAR)
        glTexParameteri(target,GL_TEXTURE_WRAP_S,GL_CLAMP_TO_EDGE); glTexParameteri(target,GL_TEXTURE_WRAP_T,GL_CLAMP_TO_EDGE)
        return names[0]
    }
    private fun reconcile(value: List<Layer>) {
        val requests = videoPlan(value)
        val videoIds = requests.keys
        val obsolete = videos.keys.filter { it !in videoIds }
        // FIFO main-thread release precedes every new player allocation. No transition decoder.
        obsolete.forEach { id ->
            val video = videos.remove(id)!!
            main.post { players.remove(id)?.player?.release(); videoAspects=videoAspects-id; video.surface.release(); video.stream.release() }
            glDeleteTextures(1,intArrayOf(video.texture),0)
        }
        val neededImages = value.filter { it.asset?.kind == "image" }.map { it.asset!!.id }.toSet()
        images.keys.filter { it !in neededImages }.forEach { id -> glDeleteTextures(1,intArrayOf(images.remove(id)!!),0) }
        val imageEdge = (2048 / sqrt(neededImages.size.coerceAtLeast(1).toDouble())).toInt().coerceAtLeast(1)
        value.forEach { layer ->
            val asset = layer.asset ?: return@forEach
            if (asset.kind == "image" && asset.id !in images) {
                val path = File(media,asset.id).absolutePath
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeFile(path,bounds)
                var sample = 1
                while (bounds.outWidth / sample > imageEdge || bounds.outHeight / sample > imageEdge) sample *= 2
                val bitmap = BitmapFactory.decodeFile(path,BitmapFactory.Options().apply { inSampleSize = sample })
                if (bitmap == null) main.post { onError("Не удалось прочитать изображение") }
                else { val texture = newTexture(GL_TEXTURE_2D); try { GLUtils.texImage2D(GL_TEXTURE_2D,0,bitmap,0); images[asset.id] = texture;imageAspects[asset.id]=bitmap.width.toFloat()/bitmap.height } finally { bitmap.recycle() } }
            }
        }
        val epoch=contextEpoch
        requests.forEach { (id,request) ->
                val asset=request.asset
                val video = videos.getOrPut(id) {
                    val texture = newTexture(GLES11Ext.GL_TEXTURE_EXTERNAL_OES)
                    val stream = SurfaceTexture(texture)
                    Video(texture,stream,Surface(stream)).also { v ->
                        stream.setOnFrameAvailableListener({ if (!closed) { v.pending.set(true); requestVideoFrame() } },main)
                    }
                }
                main.post {
                    if (closed || epoch!=contextEpoch || desired.none { !it.calibration && it.asset?.id == id }) return@post
                    var binding = players[id]
                    if (binding == null) {
                        val renderers=DefaultRenderersFactory(context).setEnableDecoderFallback(true)
                        val player = ExoPlayer.Builder(context,renderers).build()
                        player.setVideoSurface(video.surface)
                        player.repeatMode = Player.REPEAT_MODE_ONE
                        player.addListener(object : Player.Listener { override fun onPlayerError(error: PlaybackException) { Log.w("Polotno","videoError=${error.errorCode}");onError("Не удалось воспроизвести один из роликов. Попробуйте видео меньшего разрешения или менее сложную сцену") } })
                        player.addListener(object : Player.Listener { override fun onVideoSizeChanged(size:androidx.media3.common.VideoSize) { if(size.height>0)videoAspects=videoAspects+(id to (size.width.toFloat()*size.pixelWidthHeightRatio/size.height)) } })
                        player.addAnalyticsListener(object : AnalyticsListener {
                            override fun onVideoDecoderInitialized(eventTime: AnalyticsListener.EventTime, decoderName: String, initializedTimestampMs: Long, initializationDurationMs: Long) { Log.i("Polotno", "videoDecoder=$decoderName initMs=$initializationDurationMs") }
                            override fun onDroppedVideoFrames(eventTime: AnalyticsListener.EventTime, droppedFrames: Int, elapsedMs: Long) { Log.i("Polotno", "videoDropped=$droppedFrames elapsedMs=$elapsedMs") }
                            override fun onRenderedFirstFrame(eventTime: AnalyticsListener.EventTime, output: Any, renderTimeMs: Long) { Log.i("Polotno","videoFirstFrame activeDecoders=${players.size}") }
                        })
                        binding = Playback(player,""); players[id] = binding
                        Log.i("Polotno","videoPlayers=${players.size}")
                    }
                    binding.player.volume = if (request.muted) 0f else 1f
                    if (binding.assetId != asset.id) {
                        binding.player.setMediaItem(MediaItem.fromUri(android.net.Uri.fromFile(File(media,asset.id))))
                        binding.player.prepare(); binding.assetId = asset.id
                    }
                    binding.player.playWhenReady = !paused
                }
        }
        layers = value
    }
    override fun onDrawFrame(gl: GL10?) {
        val started = System.nanoTime()
        glClear(GL_COLOR_BUFFER_BIT)
        if (closed) return
        layers.forEach { layer ->
            if(layer.calibration){drawGrid(layer);layer.frame?.let{drawFrame(layer,it)};return@forEach}
            val asset = layer.asset
            if(asset==null){layer.frame?.let{drawFrame(layer,it)};return@forEach}
            val video = if (asset.kind == "video") videos[asset.id] else null
            if (video != null && video.pending.getAndSet(false)) { video.stream.updateTexImage(); video.stream.getTransformMatrix(video.transform); video.ready = true }
            val texture = video?.takeIf { it.ready }?.texture ?: if (video == null) images[asset.id] else null
            if (texture == null) {layer.frame?.let{drawFrame(layer,it)};return@forEach}
            val program = if (video == null) imageProgram else videoProgram
            glUseProgram(program)
            glUniformMatrix3fv(glGetUniformLocation(program,"projection"),1,false,Projection.matrix(layer.corners),0)
            val aspect=if(video!=null)videoAspects[asset.id]?:1f else imageAspects[asset.id]?:1f
            glUniform2f(glGetUniformLocation(program,"uvDirection"),if(layer.flipHorizontal)-1f else 1f,if(layer.flipVertical)-1f else 1f)
            glUniform2fv(glGetUniformLocation(program,"uvScale"),1,Projection.textureScale(aspect,Projection.aspect(layer.corners,viewportWidth,viewportHeight),layer.fit),0)
            if (video != null) glUniformMatrix4fv(glGetUniformLocation(program,"textureTransform"),1,false,video.transform,0)
            val position = glGetAttribLocation(program,"position"); glEnableVertexAttribArray(position)
            glVertexAttribPointer(position,2,GL_FLOAT,false,0,vertices)
            glActiveTexture(GL_TEXTURE0); glBindTexture(if (video == null) GL_TEXTURE_2D else GLES11Ext.GL_TEXTURE_EXTERNAL_OES,texture)
            glUniform1i(glGetUniformLocation(program,"content"),0)
            glDrawArrays(GL_TRIANGLES,0,6); glDisableVertexAttribArray(position)
            layer.frame?.let{drawFrame(layer,it)}
        }
        val duration = System.nanoTime()-started; totalDrawNanos += duration; draws += 1
        if (duration > 33_333_333) slowDraws += 1
    }
    fun close() {
        closed = true
        Choreographer.getInstance().removeFrameCallback(videoFrame);frameScheduled=false
        players.values.forEach { it.player.release() }; players.clear()
        view.queueEvent {
            videos.values.forEach { it.surface.release(); it.stream.release(); glDeleteTextures(1,intArrayOf(it.texture),0) }; videos.clear()
            images.values.forEach { glDeleteTextures(1,intArrayOf(it),0) }; images.clear()
            glDeleteProgram(imageProgram); glDeleteProgram(videoProgram);glDeleteProgram(frameProgram);glDeleteProgram(gridProgram)
            Log.i("Polotno", "draws=$draws slowDraws=$slowDraws meanDrawUs=${if(draws==0L)0 else totalDrawNanos/draws/1000}")
        }
    }
    private fun drawFrame(layer:Layer,frame:NeonFrame){
        val scale=viewportWidth/1920f;val margin=(frame.glow*3+frame.width)*scale
        val minX=layer.corners.minOf{it.x}*viewportWidth-margin;val maxX=layer.corners.maxOf{it.x}*viewportWidth+margin
        val minY=layer.corners.minOf{it.y}*viewportHeight-margin;val maxY=layer.corners.maxOf{it.y}*viewportHeight+margin
        fun point(x:Float,y:Float)=floatArrayOf(2*x/viewportWidth-1,1-2*y/viewportHeight)
        val values=point(minX,minY)+point(maxX,minY)+point(minX,maxY)+point(minX,maxY)+point(maxX,minY)+point(maxX,maxY)
        val buffer=ByteBuffer.allocateDirect(values.size*4).order(ByteOrder.nativeOrder()).asFloatBuffer().apply{put(values);position(0)}
        glUseProgram(frameProgram);val position=glGetAttribLocation(frameProgram,"position");glEnableVertexAttribArray(position);glVertexAttribPointer(position,2,GL_FLOAT,false,0,buffer)
        glUniform2fv(glGetUniformLocation(frameProgram,"corners[0]"),4,layer.corners.flatMap{listOf(it.x*viewportWidth,(1-it.y)*viewportHeight)}.toFloatArray(),0)
        val color=android.graphics.Color.parseColor(frame.color)
        glUniform3f(glGetUniformLocation(frameProgram,"color"),android.graphics.Color.red(color)/255f,android.graphics.Color.green(color)/255f,android.graphics.Color.blue(color)/255f)
        glUniform1f(glGetUniformLocation(frameProgram,"width"),frame.width*scale);glUniform1f(glGetUniformLocation(frameProgram,"glow"),frame.glow*scale)
        glDrawArrays(GL_TRIANGLES,0,6);glDisableVertexAttribArray(position)
    }
    private fun drawGrid(layer:Layer){
        glUseProgram(gridProgram);glUniformMatrix3fv(glGetUniformLocation(gridProgram,"projection"),1,false,Projection.matrix(layer.corners),0)
        val position=glGetAttribLocation(gridProgram,"position");glEnableVertexAttribArray(position);glVertexAttribPointer(position,2,GL_FLOAT,false,0,vertices);glDrawArrays(GL_TRIANGLES,0,6);glDisableVertexAttribArray(position)
    }
    private fun gridProgram():Int=linkedProgram("attribute vec2 position;uniform mat3 projection;varying vec2 uv;void main(){vec3 p=projection*vec3(position,1.);gl_Position=vec4(p.xy,0.,p.z);uv=position;}","""
        precision mediump float;varying vec2 uv;void main(){vec2 f=fract(uv*8.);float line=step(f.x,.025)+step(f.y,.025);float border=step(uv.x,.01)+step(.99,uv.x)+step(uv.y,.01)+step(.99,uv.y);float check=mod(floor(uv.x*8.)+floor(uv.y*8.),2.);vec3 color=vec3(.08+.05*check)+vec3(min(1.,line))*.7;if(border>0.)color=vec3(1.);if(abs(uv.x-.5)<.003||abs(uv.y-.5)<.003)color=vec3(1.,.2,.3);gl_FragColor=vec4(color,1.);}
    """)
    private fun frameProgram():Int=linkedProgram("attribute vec2 position;void main(){gl_Position=vec4(position,0.,1.);}","""
        precision highp float;uniform vec2 corners[4];uniform vec3 color;uniform float width;uniform float glow;
        float edge(vec2 p,vec2 a,vec2 b){vec2 d=b-a;return length(p-a-d*clamp(dot(p-a,d)/dot(d,d),0.,1.));}
        void main(){vec2 p=gl_FragCoord.xy;float d=min(min(edge(p,corners[0],corners[1]),edge(p,corners[1],corners[2])),min(edge(p,corners[2],corners[3]),edge(p,corners[3],corners[0])));
        float stroke=1.-smoothstep(width*.5-.5,width*.5+.5,d);float halo=glow>0.?exp(-d*d/(2.*glow*glow))*.35:0.;float alpha=min(1.,stroke+halo);vec3 c=mix(color,vec3(1.),stroke*.45);gl_FragColor=vec4(c*alpha,alpha);}
    """)
    private fun program(video: Boolean): Int {
        val vertex = """attribute vec2 position; uniform mat3 projection; varying vec2 uv;
            void main(){vec3 p=projection*vec3(position,1.0);gl_Position=vec4(p.xy,0.0,p.z);uv=position;}"""
        val fragment = if (video) """#extension GL_OES_EGL_image_external : require
            precision mediump float; varying vec2 uv; uniform samplerExternalOES content; uniform mat4 textureTransform;uniform vec2 uvScale;uniform vec2 uvDirection;
            void main(){vec2 p=(uv-.5)*uvDirection/uvScale+.5;if(any(lessThan(p,vec2(0.0)))||any(greaterThan(p,vec2(1.0))))gl_FragColor=vec4(0.,0.,0.,1.);else gl_FragColor=texture2D(content,(textureTransform*vec4(p.x,1.0-p.y,0.0,1.0)).xy);}"""
        else """precision mediump float; varying vec2 uv; uniform sampler2D content;uniform vec2 uvScale;uniform vec2 uvDirection;
            void main(){vec2 p=(uv-.5)*uvDirection/uvScale+.5;if(any(lessThan(p,vec2(0.0)))||any(greaterThan(p,vec2(1.0))))gl_FragColor=vec4(0.,0.,0.,1.);else gl_FragColor=texture2D(content,p);}"""
        return linkedProgram(vertex,fragment)
    }
    private fun linkedProgram(vertex:String,fragment:String):Int{
        fun shader(type: Int, source: String): Int {
            val shader = glCreateShader(type); glShaderSource(shader,source); glCompileShader(shader)
            val status = IntArray(1); glGetShaderiv(shader,GL_COMPILE_STATUS,status,0)
            check(status[0] != 0) { glGetShaderInfoLog(shader) }; return shader
        }
        val vs = shader(GL_VERTEX_SHADER,vertex); val fs = shader(GL_FRAGMENT_SHADER,fragment)
        val program = glCreateProgram(); glAttachShader(program,vs); glAttachShader(program,fs); glLinkProgram(program)
        glDeleteShader(vs); glDeleteShader(fs)
        val status = IntArray(1); glGetProgramiv(program,GL_LINK_STATUS,status,0); check(status[0] != 0) { glGetProgramInfoLog(program) }
        return program
    }
}
