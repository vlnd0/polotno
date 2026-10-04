package app.polotno

import kotlin.math.abs

/** Homography from unit texture coordinates to a convex screen quad. Column-major GLES matrix. */
object Projection {
    fun aspect(corners: List<Point>, width: Int, height: Int): Float {
        fun distance(a:Point,b:Point)=kotlin.math.hypot((a.x-b.x)*width,(a.y-b.y)*height)
        return (distance(corners[0],corners[1])+distance(corners[2],corners[3]))/(distance(corners[1],corners[2])+distance(corners[3],corners[0]))
    }
    fun textureScale(mediaAspect:Float,areaAspect:Float,fit:String):FloatArray {
        if(fit=="stretch")return floatArrayOf(1f,1f)
        val ratio=mediaAspect/areaAspect
        return if(fit=="cover") {if(ratio>1)floatArrayOf(ratio,1f) else floatArrayOf(1f,1f/ratio)} else {if(ratio>1)floatArrayOf(1f,1f/ratio) else floatArrayOf(ratio,1f)}
    }
    fun matrix(corners: List<Point>): FloatArray {
        require(corners.size == 4)
        val p = corners.map { Point(2 * it.x - 1, 1 - 2 * it.y) }
        val dx1 = p[1].x - p[2].x; val dx2 = p[3].x - p[2].x
        val dy1 = p[1].y - p[2].y; val dy2 = p[3].y - p[2].y
        val dx3 = p[0].x - p[1].x + p[2].x - p[3].x
        val dy3 = p[0].y - p[1].y + p[2].y - p[3].y
        val affine = abs(dx3) + abs(dy3) < 1e-6f
        val determinant = dx1 * dy2 - dx2 * dy1
        require(affine || abs(determinant) > 1e-8f) { "Вырожденная область" }
        val g = if (affine) 0f else (dx3 * dy2 - dx2 * dy3) / determinant
        val h = if (affine) 0f else (dx1 * dy3 - dx3 * dy1) / determinant
        return floatArrayOf(p[1].x - p[0].x + g * p[1].x, p[1].y - p[0].y + g * p[1].y, g,
            p[3].x - p[0].x + h * p[3].x, p[3].y - p[0].y + h * p[3].y, h,
            p[0].x, p[0].y, 1f)
    }
}
