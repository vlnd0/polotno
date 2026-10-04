package app.polotno

import org.junit.Assert.*
import org.junit.Test

class ProjectionTest {
    @Test fun portraitContainmentPreservesAspect(){
        assertArrayEquals(floatArrayOf(81f/256f,1f),Projection.textureScale(9f/16f,16f/9f,"contain"),1e-6f)
        assertArrayEquals(floatArrayOf(1f,256f/81f),Projection.textureScale(9f/16f,16f/9f,"cover"),1e-6f)
    }
    @Test fun mapsAllFourCornersWithPerspective() {
        val corners = listOf(Point(.1f,.2f),Point(.8f,.1f),Point(.9f,.9f),Point(.2f,.7f))
        val m = Projection.matrix(corners)
        val uv = listOf(0f to 0f,1f to 0f,1f to 1f,0f to 1f)
        uv.forEachIndexed { i, (u,v) ->
            val w=m[2]*u+m[5]*v+m[8]
            assertEquals(2*corners[i].x-1,(m[0]*u+m[3]*v+m[6])/w,1e-5f)
            assertEquals(1-2*corners[i].y,(m[1]*u+m[4]*v+m[7])/w,1e-5f)
        }
    }
    @Test fun mapsAffineRectangle() {
        val m=Projection.matrix(listOf(Point(0f,0f),Point(1f,0f),Point(1f,1f),Point(0f,1f)))
        assertArrayEquals(floatArrayOf(2f,0f,0f,0f,-2f,0f,-1f,1f,1f),m,1e-6f)
    }
    @Test fun independentClocksSurviveGeometryAndOrderChangesAndPause() {
        val a=Region("a",emptyList(),listOf("1","2"),1000,true)
        val b=Region("b",emptyList(),listOf("3","4"),2000,true)
        val clock=PlaylistClock()
        fun state(regions:List<Region> = listOf(a,b),paused:Boolean=false)=Snapshot("scene",1,paused,regions,emptyMap())
        assertEquals(mapOf("a" to "1","b" to "3"),clock.items(state(),0))
        assertEquals(mapOf("a" to "2","b" to "3"),clock.items(state(listOf(b,a.copy(corners=listOf(Point(.1f,.1f))))),1100))
        clock.items(state(paused=true),1200)
        assertEquals(mapOf("a" to "2","b" to "3"),clock.items(state(paused=true),9200))
        assertEquals(mapOf("a" to "2","b" to "3"),clock.items(state(),9200))
        assertEquals(mapOf("a" to "1","b" to "4"),clock.items(state(),10000))
    }
}
