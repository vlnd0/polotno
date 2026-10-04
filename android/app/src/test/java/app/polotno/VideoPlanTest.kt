package app.polotno

import org.junit.Assert.*
import org.junit.Test

class VideoPlanTest {
    private fun layer(id:String,asset:String,muted:Boolean=true,kind:String="video",grid:Boolean=false)=
        Layer(id,listOf(Point(0f,0f),Point(1f,0f),Point(1f,1f),Point(0f,1f)),Asset(asset,kind),muted,calibration=grid)

    @Test fun duplicateVideoSharesDecoderAndAudioIsPlayedOnce() {
        val layers=listOf(layer("a","clip"),layer("b","clip",false).copy(flipHorizontal=true),layer("c","clip").copy(flipVertical=true))
        assertEquals(1,videoPlan(layers).size)
        assertFalse(videoPlan(layers).getValue("clip").muted)
        assertTrue(videoPlan(layers.filter { it.id!="b" }).getValue("clip").muted)
    }
    @Test fun distinctClipsHaveNoArtificialCapAndRemovedSourcesDisappear() {
        val layers=(1..12).map { layer("region$it","clip$it") }
        assertEquals(12,videoPlan(layers).size)
        assertEquals(setOf("clip1","clip2"),videoPlan(layers.take(2)).keys)
        assertTrue(videoPlan(emptyList()).isEmpty())
    }
    @Test fun calibrationAndImagesDoNotAllocateVideoDecoders() {
        assertTrue(videoPlan(listOf(layer("grid","clip",grid=true),layer("picture","image",kind="image"))).isEmpty())
    }
}
